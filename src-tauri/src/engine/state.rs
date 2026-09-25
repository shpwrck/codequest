use super::*;

pub(super) const HERO_NAMES: [&str; 6] = ["SUDO", "GREP", "VIM", "FORK", "ASYNC", "PATCH"];
pub(super) const HERO_CLASSES: [&str; 6] = [
    "CODE KNIGHT",
    "BUG MAGE",
    "PIPE MONK",
    "MERGE PALADIN",
    "LINT RANGER",
    "SHELL DRUID",
];
pub(super) const HERO_STYLES: [&str; 5] = ["EMBER", "OCEAN", "FOREST", "GOLD", "VOID"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Screen {
    Off,
    Boot,
    Copyright,
    OpeningFanfare,
    Title,
    QuizMenu,
    CharacterCreation,
    Oracle,
    Quiz,
    LevelUp,
    GameOver,
    Codex,
    QuestSelect,
    Battle,
    Victory,
    Defeat,
}

impl From<SceneHandler> for Screen {
    fn from(handler: SceneHandler) -> Self {
        match handler {
            SceneHandler::RepositoryCredits => Self::Copyright,
            SceneHandler::OpeningFanfare => Self::OpeningFanfare,
            SceneHandler::Title => Self::Title,
            SceneHandler::QuizMenu => Self::QuizMenu,
            SceneHandler::CharacterCreation => Self::CharacterCreation,
            SceneHandler::Oracle => Self::Oracle,
            SceneHandler::ConceptQuiz => Self::Quiz,
            SceneHandler::LevelUp => Self::LevelUp,
            SceneHandler::GameOver => Self::GameOver,
            SceneHandler::Codex => Self::Codex,
            SceneHandler::QuestSelect => Self::QuestSelect,
            SceneHandler::Battle => Self::Battle,
            SceneHandler::Victory => Self::Victory,
            SceneHandler::Defeat => Self::Defeat,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OpeningBeat {
    Legacy,
    SourceEmber,
    ArchiveAnswer,
    MemoryVault,
    Convergence,
    OracleAwakening,
}

#[derive(Resource)]
pub(super) struct GameState {
    pub(super) powered: bool,
    /// Mirrors the host's reduced-motion preference for decorative motion.
    pub(super) reduced_motion: bool,
    pub(super) ai_provider: Option<String>,
    pub(super) cartridge: Option<CartridgeSpec>,
    pub(super) machine: Option<SceneMachine>,
    pub(super) screen: Screen,
    pub(super) screen_ticks: u64,
    pub(super) held: HashSet<Button>,
    pub(super) menu_selected: usize,
    /// Codex page: 0 is the mastery overview, `n` is journal lesson `n - 1`.
    pub(super) codex_page: usize,
    /// True once A revealed the current Codex page's pending answer. Every
    /// page turn and every visit seals it again.
    pub(super) codex_revealed: bool,
    pub(super) hero_row: usize,
    pub(super) hero_name: usize,
    pub(super) hero_class: usize,
    pub(super) hero_style: usize,
    pub(super) quest_selected: usize,
    pub(super) quiz: Option<QuizRun>,
    pub(super) batch_ends: Vec<usize>,
    /// Generation level of each batch, parallel to `batch_ends`.
    pub(super) batch_levels: Vec<u32>,
    /// Same-launch review copies whose retry gap reaches past the deck's
    /// end, each with the deck index it is due at. The next delivery places
    /// them; a new run drops them and requeues their outstanding lessons.
    pub(super) deferred_retries: Vec<(usize, QuizQuestion)>,
    /// A delivery held until a safe screen boundary: cartridge id, questions,
    /// and the level they were requested at.
    pub(super) pending_questions: Option<(String, Vec<QuizQuestion>, u32)>,
    pub(super) questions_loading: bool,
    pub(super) question_retry_ticks: u16,
    /// Ticks the request in flight has been out, so a retry after a failure
    /// keeps the failure on the Oracle line until it has had time to answer.
    pub(super) question_request_ticks: u16,
    /// The `screen_ticks` at which the Oracle line last became a recall, so
    /// every recall starts at the top of the recall order, written in fresh.
    pub(super) oracle_recall_since: u64,
    /// Sequence number of the newest question request; only its reply lands.
    pub(super) question_request_seq: u64,
    /// The level the newest question request asked for.
    pub(super) question_request_level: u32,
    /// Why the newest answered question request failed, as the Oracle line
    /// shows it; cleared when a request succeeds or a cartridge is inserted.
    pub(super) question_failure: Option<String>,
    /// Deck prefix committed in this session; the next run drops it.
    pub(super) consumed_questions: usize,
    pub(super) oracle_hero_x: i32,
    pub(super) oracle_drops: Vec<OracleDrop>,
    pub(super) oracle_spawned: u32,
    pub(super) oracle_data: u32,
    pub(super) oracle_bug_hits: u32,
    pub(super) logs: VecDeque<(String, bool)>,
    /// Generation of the newest quest started; only its messages land.
    pub(super) quest_generation: u64,
    pub(super) active_boss: String,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            powered: false,
            reduced_motion: false,
            ai_provider: None,
            cartridge: None,
            machine: None,
            screen: Screen::Off,
            screen_ticks: 0,
            held: HashSet::new(),
            menu_selected: 0,
            codex_page: 0,
            codex_revealed: false,
            hero_row: 0,
            hero_name: 0,
            hero_class: 0,
            hero_style: 0,
            quest_selected: 0,
            quiz: None,
            batch_ends: Vec::new(),
            batch_levels: Vec::new(),
            deferred_retries: Vec::new(),
            pending_questions: None,
            questions_loading: false,
            question_retry_ticks: 0,
            question_request_ticks: 0,
            oracle_recall_since: 0,
            question_request_seq: 0,
            question_request_level: 1,
            question_failure: None,
            consumed_questions: 0,
            oracle_hero_x: 104,
            oracle_drops: Vec::new(),
            oracle_spawned: 0,
            oracle_data: 0,
            oracle_bug_hits: 0,
            logs: VecDeque::new(),
            quest_generation: 0,
            active_boss: String::new(),
        }
    }
}

impl GameState {
    pub(super) fn ai_provider_name(&self) -> &str {
        self.ai_provider.as_deref().unwrap_or("AI")
    }

    /// Clock for decorative motion (bobbing, pulsing, twinkling, scrolling).
    /// Reduced motion freezes it at the resting composition, while scene timing,
    /// input, and data keep using `screen_ticks`.
    pub(super) fn motion_ticks(&self) -> u64 {
        if self.reduced_motion {
            0
        } else {
            self.screen_ticks
        }
    }

    /// Progress of a settling motion capped at `cap` ticks; reduced motion shows
    /// its settled end state immediately.
    pub(super) fn settled_ticks(&self, cap: u64) -> u64 {
        if self.reduced_motion {
            cap
        } else {
            self.screen_ticks.min(cap)
        }
    }

    /// Whether a blinking prompt is lit this tick; reduced motion keeps it lit.
    pub(super) fn blink_lit(&self, period: u64) -> bool {
        self.reduced_motion || (self.screen_ticks / period).is_multiple_of(2)
    }

    pub(super) fn ai_provider_status(&self, status: &str) -> String {
        format!("{}:{status}", self.ai_provider_name())
    }

    pub(super) fn transition(&mut self, screen: Screen) {
        if screen == Screen::Oracle && self.screen != Screen::Oracle {
            self.oracle_hero_x = 104;
            self.oracle_drops.clear();
            self.oracle_spawned = 0;
        }
        if screen == Screen::Codex && self.screen != Screen::Codex {
            self.codex_page = 0;
            self.codex_revealed = false;
        }
        // Entering the menu focuses BEGIN (so Title then A cannot bounce back to
        // Title), except when returning from the Codex, which keeps its option.
        if screen == Screen::QuizMenu && !matches!(self.screen, Screen::QuizMenu | Screen::Codex) {
            self.menu_selected = 0;
        }
        // Whatever route the scene graph takes into the Oracle or the quiz,
        // they always run inside a live run. A run ends as soon as the graph
        // leaves the run's screens by any route (the next run retires its
        // answers there), so entering from outside them starts a fresh one.
        let within_run = matches!(self.screen, Screen::Oracle | Screen::Quiz | Screen::LevelUp);
        if matches!(screen, Screen::Oracle | Screen::Quiz)
            && (!within_run || self.quiz.as_ref().is_none_or(|run| run.hearts == 0))
        {
            start_quiz_run(self);
        }
        self.screen = screen;
        self.screen_ticks = 0;
        self.oracle_recall_since = 0;
    }

    pub(super) fn start_machine(&mut self) {
        let handler = self.machine.as_mut().map(|machine| {
            machine.reset();
            machine.current_handler()
        });
        if let Some(handler) = handler {
            self.transition(handler.into());
        }
    }

    pub(super) fn signal(&mut self, signal: SceneSignal) -> bool {
        let change = self
            .machine
            .as_mut()
            .and_then(|machine| machine.handle(SceneEvent::Signal(signal)));
        if let Some(change) = change {
            self.transition(change.handler.into());
            true
        } else {
            false
        }
    }

    pub(super) fn tick_machine(&mut self) -> bool {
        let change = self
            .machine
            .as_mut()
            .and_then(|machine| machine.handle(SceneEvent::Tick));
        if let Some(change) = change {
            self.transition(change.handler.into());
            true
        } else {
            false
        }
    }

    pub(super) fn can_signal(&self, signal: SceneSignal) -> bool {
        self.machine
            .as_ref()
            .is_some_and(|machine| machine.can_handle(signal))
    }

    pub(super) fn has_game(&self) -> bool {
        self.cartridge.is_some()
    }

    pub(super) fn cartridge_mode(&self) -> Option<CartridgeMode> {
        self.cartridge.as_ref().map(CartridgeSpec::mode)
    }

    pub(super) fn question_count(&self) -> usize {
        self.cartridge
            .as_ref()
            .map_or(0, |cart| cart.questions.len())
    }

    pub(super) fn has_unanswered_question(&self) -> bool {
        let question = self.quiz.as_ref().map_or(0, |run| run.question);
        self.cartridge
            .as_ref()
            .is_some_and(|cartridge| cartridge.questions.get(question).is_some())
    }

    /// The Oracle's truthful question status. Every Datafall status line, the
    /// audio snapshot, and the screen transcript read this one rule.
    pub(super) fn question_status(&self) -> QuestionStatus {
        if self.has_unanswered_question() {
            QuestionStatus::Ready
        } else if self.questions_loading {
            QuestionStatus::Writing
        } else if self.question_retry_ticks > 0 {
            QuestionStatus::Retrying
        } else {
            QuestionStatus::Contacting
        }
    }

    /// True while a run is being played: the Oracle, quiz, and level-up
    /// screens with a run in hand. Every other screen precedes the next run.
    pub(super) fn run_is_live(&self) -> bool {
        self.quiz.is_some()
            && matches!(self.screen, Screen::Oracle | Screen::Quiz | Screen::LevelUp)
    }

    /// Whether the question the player faces next exists: the live run's
    /// current question, or anything the next run keeps from the deck.
    pub(super) fn has_next_question(&self) -> bool {
        if self.run_is_live() {
            self.has_unanswered_question()
        } else {
            self.question_count() > self.consumed_questions
        }
    }

    pub(super) fn batch_start(&self, index: usize) -> usize {
        index
            .checked_sub(1)
            .and_then(|previous| self.batch_ends.get(previous).copied())
            .unwrap_or(0)
    }

    /// Whether batch `index` holds `QUESTION_BATCH_SIZE` new questions. Review
    /// copies ride along in their batch but never count toward it.
    pub(super) fn batch_is_full(&self, index: usize) -> bool {
        let questions = self
            .cartridge
            .as_ref()
            .map_or(&[][..], |cartridge| cartridge.questions.as_slice());
        self.batch_ends.get(index).is_some_and(|end| {
            new_question_count(questions, self.batch_start(index), *end) >= QUESTION_BATCH_SIZE
        })
    }

    /// The level the next requested batch should be generated at: never
    /// below the level the run will have reached when those questions start
    /// (one level-up per full batch still ahead), and never a level already
    /// queued. An open (short) last batch is topped up at its own level.
    pub(super) fn next_batch_level(&self) -> u32 {
        let (run_level, completed) = self
            .quiz
            .as_ref()
            .filter(|_| self.run_is_live())
            .map_or((1, 0), |run| (run.level, run.completed_batches));
        let full_ahead = (completed..self.batch_ends.len())
            .filter(|index| self.batch_is_full(*index))
            .count() as u32;
        let queued = self.batch_ends.len().checked_sub(1).map_or(0, |last| {
            let level = self.batch_levels.get(last).copied().unwrap_or(1);
            if self.batch_is_full(last) {
                level.saturating_add(1)
            } else {
                level
            }
        });
        run_level.saturating_add(full_ahead).max(queued)
    }

    /// The generation level of the batch the player enters next: its queued
    /// level, else the level of the request in flight for it, else the level
    /// the next request will ask for (never below the run's). Saved batches
    /// can sit above the run's level, so this, not `run.level`, names the
    /// lenses coming up.
    pub(super) fn upcoming_batch_level(&self) -> u32 {
        let (completed, run_level) = self
            .quiz
            .as_ref()
            .map_or((0, 1), |run| (run.completed_batches, run.level));
        self.batch_levels
            .get(completed)
            .copied()
            .or_else(|| {
                self.questions_loading
                    .then_some(self.question_request_level)
            })
            .unwrap_or_else(|| self.next_batch_level().max(run_level))
    }

    /// The signal a level-up continues with: back to the quiz when the next
    /// question is ready, otherwise to the Oracle to wait for it.
    pub(super) fn level_up_signal(&self) -> SceneSignal {
        if self.has_unanswered_question() {
            SceneSignal::QuestionsReady
        } else {
            SceneSignal::NeedsQuestion
        }
    }

    /// Whether the scene graph accepts a level-up continuation right now.
    pub(super) fn level_up_can_continue(&self) -> bool {
        self.can_signal(self.level_up_signal())
    }

    pub(super) fn presentation_tier(&self) -> PresentationTier {
        PresentationTier::from_level(self.quiz.as_ref().map_or(1, |run| run.level))
    }

    pub(super) fn visual_tier(&self) -> PresentationTier {
        if self.has_visual_template(VisualTemplate::Progression) {
            self.presentation_tier()
        } else {
            PresentationTier::Initiate
        }
    }

    pub(super) fn has_visual_template(&self, template: VisualTemplate) -> bool {
        self.cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.codequest.as_deref())
            .is_some_and(|config| config.art.iter().any(|art| art.template == Some(template)))
    }

    pub(super) fn uses_visual_template(&self, template: VisualTemplate) -> bool {
        let Some(config) = self
            .cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.codequest.as_deref())
        else {
            return false;
        };
        let Some(scene_id) = self.machine.as_ref().map(SceneMachine::current_scene) else {
            return false;
        };
        let Some(scene) = config.scenes.iter().find(|scene| scene.id == scene_id) else {
            return false;
        };
        scene.art.iter().any(|art_id| {
            config
                .art
                .iter()
                .any(|art| art.id == *art_id && art.template == Some(template))
        })
    }

    pub(super) fn uses_art(&self, expected_art_id: &str) -> bool {
        let Some(config) = self
            .cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.codequest.as_deref())
        else {
            return false;
        };
        let Some(scene_id) = self.machine.as_ref().map(SceneMachine::current_scene) else {
            return false;
        };
        config
            .scenes
            .iter()
            .find(|scene| scene.id == scene_id)
            .is_some_and(|scene| scene.art.iter().any(|art_id| art_id == expected_art_id))
    }

    pub(super) fn declares_art(&self, expected_art_id: &str) -> bool {
        self.cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.codequest.as_deref())
            .is_some_and(|config| config.art.iter().any(|art| art.id == expected_art_id))
    }

    pub(super) fn opening_beat(&self) -> OpeningBeat {
        if self.uses_art("opening-source") {
            OpeningBeat::SourceEmber
        } else if self.uses_art("opening-signal") {
            OpeningBeat::ArchiveAnswer
        } else if self.uses_art("opening-archive") {
            OpeningBeat::MemoryVault
        } else if self.uses_art("opening-convergence") {
            OpeningBeat::Convergence
        } else if self.uses_art("opening-fanfare") && self.declares_art("opening-source") {
            OpeningBeat::OracleAwakening
        } else {
            OpeningBeat::Legacy
        }
    }

    pub(super) fn lens_record(&self, concept: Concept) -> LensRecord {
        self.cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.mastery.get(&concept))
            .copied()
            .unwrap_or_default()
    }

    /// Runes the lens's evidence volume earned that its gates hold back,
    /// drawn cracked beside the lit ones.
    pub(super) fn mastery_cracks(&self, concept: Concept) -> usize {
        self.lens_record(concept)
            .cracks_with(pending_reviews(self.lessons(), concept))
    }

    /// What the cracked runes mean, while any lens shows one: a review is due
    /// when a cracked lens has a pending review, and otherwise only its recent
    /// accuracy holds the rune back, so no review exists to promise.
    pub(super) fn mastery_crack_legend(&self) -> Option<&'static str> {
        let mut cracked = false;
        for concept in Concept::ALL {
            if self.mastery_cracks(concept) > 0 {
                if pending_reviews(self.lessons(), concept) > 0 {
                    return Some(CODEX_CRACKED_LEGEND);
                }
                cracked = true;
            }
        }
        cracked.then_some(CODEX_SLIPPED_LEGEND)
    }

    /// The current question's place in its batch and the batch's length,
    /// which grows as misses insert their review copies. An open (short) last
    /// batch counts toward the full `QUESTION_BATCH_SIZE` new questions it
    /// will be topped up to, plus the review copies it already holds, so the
    /// header never promises a batch end that is not coming and a miss grows
    /// it at once. Both cap at 99.
    pub(super) fn batch_progress(&self) -> Option<(usize, usize)> {
        let run = self.quiz.as_ref()?;
        let batch = run.completed_batches;
        let start = self.batch_start(batch);
        let end = *self.batch_ends.get(batch)?;
        let questions = self
            .cartridge
            .as_ref()
            .map_or(&[][..], |cartridge| cartridge.questions.as_slice());
        let reviews = questions
            .iter()
            .take(end)
            .skip(start)
            .filter(|question| question.review.is_review())
            .count();
        let length = reviews + (end - start - reviews).max(QUESTION_BATCH_SIZE);
        (start..end)
            .contains(&run.question)
            .then(|| ((run.question - start + 1).min(99), length.min(99)))
    }

    /// Ticks the Oracle line has been a recall, counted from the tick it last
    /// took the line over (or from the screen's start).
    pub(super) fn oracle_recall_ticks(&self) -> u64 {
        self.screen_ticks.saturating_sub(self.oracle_recall_since)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_status_uses_the_installed_battery_provider() {
        let mut state = GameState::default();
        assert_eq!(state.ai_provider_status("SCRYING"), "AI:SCRYING");

        state.ai_provider = Some("CODEX".into());
        assert_eq!(state.ai_provider_status("READY"), "CODEX:READY");

        state.ai_provider = Some("CLAUDE".into());
        assert_eq!(state.ai_provider_status("CLOUDY"), "CLAUDE:CLOUDY");
    }

    #[test]
    fn leaving_the_run_by_any_manifest_route_ends_it() {
        use SceneHandler as H;
        use SceneSignal as S;
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..2 * QUESTION_BATCH_SIZE).map(concept_question).collect();
        cartridge.machine = Box::new(
            SceneMachineDefinition::compile(
                "menu",
                vec![
                    scene_spec(
                        "menu",
                        H::QuizMenu,
                        &[(S::NewRun, "oracle", None), (S::Back, "menu", None)],
                    ),
                    scene_spec(
                        "oracle",
                        H::Oracle,
                        &[(S::QuestionsReady, "quiz", None), (S::Back, "menu", None)],
                    ),
                    scene_spec(
                        "quiz",
                        H::ConceptQuiz,
                        &[
                            (S::NeedsQuestion, "oracle", None),
                            (S::BatchComplete, "menu", None),
                            (S::HeartsEmpty, "menu", None),
                            (S::Back, "menu", None),
                        ],
                    ),
                ],
            )
            .unwrap(),
        );
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        press(&mut engine, Button::A);
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        for _ in 0..QUESTION_BATCH_SIZE {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        assert_eq!(engine.screen(), Screen::QuizMenu);
        engine.update();

        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Oracle);
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(
            engine.screen(),
            Screen::Quiz,
            "the ready questions are asked"
        );
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert_eq!(
            (run.question, run.level, run.completed_batches, run.score),
            (0, 1, 0, 0),
            "a fresh run"
        );
        assert_eq!(
            current_question(&engine).question,
            concept_question(QUESTION_BATCH_SIZE).question
        );
    }

    #[test]
    fn returning_to_the_quiz_menu_focuses_begin() {
        let mut engine = playing_quiz_engine();
        press(&mut engine, Button::B);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        // Leave the menu with its second option focused; coming back through
        // the title must not keep that stale focus.
        press(&mut engine, Button::Down);
        assert_eq!(engine_state(&engine).menu_selected, 1);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::Title);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        assert_eq!(engine_state(&engine).menu_selected, 0);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::CharacterCreation);
    }

    #[test]
    fn a_scene_graph_that_skips_hero_creation_still_plays_a_fresh_run() {
        use SceneHandler as H;
        use SceneSignal as S;
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..QUESTION_BATCH_SIZE).map(concept_question).collect();
        cartridge.machine = Box::new(
            SceneMachineDefinition::compile(
                "menu",
                vec![
                    scene_spec(
                        "menu",
                        H::QuizMenu,
                        &[(S::NewRun, "oracle", None), (S::Back, "menu", None)],
                    ),
                    scene_spec(
                        "oracle",
                        H::Oracle,
                        &[(S::QuestionsReady, "quiz", None), (S::Back, "menu", None)],
                    ),
                    scene_spec(
                        "quiz",
                        H::ConceptQuiz,
                        &[
                            (S::NeedsQuestion, "oracle", None),
                            (S::HeartsEmpty, "menu", None),
                            (S::Back, "menu", None),
                        ],
                    ),
                ],
            )
            .unwrap(),
        );
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        assert_eq!(engine.screen(), Screen::QuizMenu);

        for run in 0..2 {
            press(&mut engine, Button::A);
            assert_eq!(engine.screen(), Screen::Oracle, "run {run}");
            for _ in 0..75 {
                engine.update();
            }
            assert_eq!(engine.screen(), Screen::Quiz, "run {run}");
            let run_state = engine_state(&engine).quiz.as_ref().unwrap();
            assert_eq!((run_state.question, run_state.hearts), (0, 3));
            let _ = engine.take_effects();
            commit(&mut engine, true);
            assert!(engine.take_effects().iter().any(|effect| matches!(
                effect,
                EngineEffect::RecordAnsweredQuestion { evidence, .. } if evidence.correct
            )));
            finish_lesson(&mut engine);
            press(&mut engine, Button::B);
            press(&mut engine, Button::B);
            assert_eq!(engine.screen(), Screen::QuizMenu, "run {run}");
            assert!(engine_state(&engine).quiz.is_none(), "Back clears the run");
        }
        // The second run started on the question after the first run's answer.
        assert_eq!(
            engine_state(&engine).question_count(),
            QUESTION_BATCH_SIZE - 2
        );

        // Even a quiz entered without a run answers B.
        {
            let mut state = engine.app.world_mut().resource_mut::<GameState>();
            state.machine.as_mut().unwrap().reset();
            state.signal(SceneSignal::NewRun);
            state.signal(SceneSignal::QuestionsReady);
            state.quiz = None;
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
    }

    #[test]
    fn cartridge_machine_controls_runtime_scene_order() {
        let machine = SceneMachineDefinition::compile(
            "title",
            vec![
                SceneSpec {
                    id: "title".into(),
                    handler: SceneHandler::Title,
                    transitions: vec![SceneTransition {
                        signal: SceneSignal::Continue,
                        target: "game-over".into(),
                        after_ticks: None,
                    }],
                },
                SceneSpec {
                    id: "game-over".into(),
                    handler: SceneHandler::GameOver,
                    transitions: vec![],
                },
            ],
        )
        .unwrap();
        let mut cartridge = quiz_cartridge();
        cartridge.machine = Box::new(machine);
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        assert_eq!(engine.screen(), Screen::Title);

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        assert_eq!(engine.screen(), Screen::GameOver);
    }

    #[test]
    fn opening_scenes_auto_advance_to_title() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);

        for _ in 0..179 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::OpeningFanfare);

        for _ in 0..330 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Title);
    }

    #[test]
    fn oracle_opening_auto_advances_through_five_story_scenes() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(oracle_template_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);

        for _ in 0..179 {
            engine.update();
        }
        let scene_id = |engine: &GameEngine| {
            engine
                .app
                .world()
                .resource::<GameState>()
                .machine
                .as_ref()
                .expect("the cartridge should own a scene machine")
                .current_scene()
                .to_string()
        };
        assert_eq!(scene_id(&engine), "opening-fanfare");

        for (ticks, expected_scene) in [
            (96, "archive-answer"),
            (66, "memory-vault"),
            (66, "convergence"),
            (66, "oracle-awakening"),
            (66, "title"),
        ] {
            for _ in 0..ticks {
                engine.update();
            }
            assert_eq!(scene_id(&engine), expected_scene);
        }
        assert_eq!(engine.screen(), Screen::Title);
    }

    #[test]
    fn single_scene_oracle_opening_keeps_the_legacy_luminance_timeline() {
        let config = CodeQuestConfig::parse(
            r#"
                schema_version = 2

                [game]
                type = "quiz"
                start_scene = "opening-fanfare"

                [[art]]
                id = "opening-fanfare"
                kind = "scene-vfx"
                summary = "One-plate Oracle opening."
                template = "oracle-awakening"

                [[scenes]]
                id = "opening-fanfare"
                title = "Opening Fanfare"
                kind = "cinematic"
                handler = "opening-fanfare"
                art = ["opening-fanfare"]

                [[scenes.transitions]]
                signal = "elapsed"
                target = "title"
                after_ticks = 330

                [[scenes]]
                id = "title"
                title = "Title"
                kind = "title"
                handler = "title"
            "#,
        )
        .expect("the legacy single-scene fixture should parse");
        let mut cartridge = quiz_cartridge();
        cartridge.machine = Box::new(
            config
                .runtime_machine()
                .expect("the fixture scene graph should compile")
                .expect("schema v2 should produce a machine"),
        );
        cartridge.codequest = Some(Box::new(config));
        let state = GameState {
            machine: Some(SceneMachine::new((*cartridge.machine).clone())),
            cartridge: Some(cartridge),
            ..Default::default()
        };

        assert_eq!(state.opening_beat(), OpeningBeat::Legacy);
    }

    #[test]
    fn reduced_motion_freezes_decorative_motion_but_keeps_scene_timing() {
        fn sampled(engine: &mut GameEngine, samples: usize, spacing: usize) -> Vec<Vec<u8>> {
            (0..samples)
                .map(|_| {
                    for _ in 0..spacing {
                        engine.update();
                    }
                    engine.frame().to_vec()
                })
                .collect()
        }
        let animates = |frames: &[Vec<u8>]| frames.windows(2).any(|pair| pair[0] != pair[1]);

        for reduced in [false, true] {
            let mut engine = GameEngine::new();
            issue(&mut engine, EngineCommand::ReducedMotion(reduced));
            issue(
                &mut engine,
                EngineCommand::Cartridge(Some(oracle_template_cartridge())),
            );
            issue(&mut engine, EngineCommand::Power(true));
            finish_opening(&mut engine);
            let title = sampled(&mut engine, 4, 17);
            press(&mut engine, Button::Start);
            press(&mut engine, Button::A);
            assert_eq!(engine.screen(), Screen::CharacterCreation);
            let atelier = sampled(&mut engine, 4, 23);

            assert_eq!(
                animates(&title),
                !reduced,
                "title prompt (reduced={reduced})"
            );
            assert_eq!(animates(&atelier), !reduced, "hero bob (reduced={reduced})");
        }

        let scene_timeline = |reduced: bool| {
            let mut engine = GameEngine::new();
            issue(&mut engine, EngineCommand::ReducedMotion(reduced));
            issue(
                &mut engine,
                EngineCommand::Cartridge(Some(oracle_template_cartridge())),
            );
            issue(&mut engine, EngineCommand::Power(true));
            issue(&mut engine, EngineCommand::BootComplete);
            (0..720)
                .map(|_| {
                    engine.update();
                    engine.screen()
                })
                .collect::<Vec<_>>()
        };
        let timeline = scene_timeline(true);
        assert_eq!(
            timeline,
            scene_timeline(false),
            "motion never changes timing"
        );
        assert_eq!(timeline.last(), Some(&Screen::Title));
    }

    #[test]
    fn a_broken_ward_routed_back_to_the_oracle_starts_a_fresh_run() {
        use SceneHandler as H;
        use SceneSignal as S;
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..QUESTION_BATCH_SIZE).map(concept_question).collect();
        cartridge.machine = Box::new(
            SceneMachineDefinition::compile(
                "menu",
                vec![
                    scene_spec(
                        "menu",
                        H::QuizMenu,
                        &[(S::NewRun, "oracle", None), (S::Back, "menu", None)],
                    ),
                    scene_spec(
                        "oracle",
                        H::Oracle,
                        &[(S::QuestionsReady, "quiz", None), (S::Back, "menu", None)],
                    ),
                    scene_spec(
                        "quiz",
                        H::ConceptQuiz,
                        &[
                            (S::NeedsQuestion, "oracle", None),
                            (S::HeartsEmpty, "oracle", None),
                            (S::Back, "menu", None),
                        ],
                    ),
                ],
            )
            .unwrap(),
        );
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        press(&mut engine, Button::A);
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);

        for _ in 0..3 {
            commit(&mut engine, false);
            finish_lesson(&mut engine);
        }
        assert_eq!(engine.screen(), Screen::Oracle);
        let run = engine_state(&engine).quiz.as_ref().unwrap();
        assert_eq!(
            (run.hearts, run.question, run.ledger.first_try),
            (3, 0, 0),
            "a dead run never carries into the Oracle"
        );
    }

    #[test]
    fn a_run_that_is_not_live_does_not_set_the_next_batch_level() {
        let mut state = GameState {
            cartridge: Some(quiz_cartridge()),
            quiz: Some(QuizRun {
                level: 3,
                completed_batches: 2,
                ..QuizRun::new()
            }),
            ..Default::default()
        };
        assert!(!state.run_is_live());
        assert_eq!(
            state.next_batch_level(),
            1,
            "the next run starts at Initiate, whatever the last one reached"
        );
        state.screen = Screen::Quiz;
        assert_eq!(state.next_batch_level(), 3);
    }
}
