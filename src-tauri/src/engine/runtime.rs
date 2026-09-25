use super::*;

pub(super) const FRAME_TIME: Duration = Duration::from_nanos(16_666_667);

pub struct GameEngine {
    pub(super) app: App,
}

impl GameEngine {
    pub fn new() -> Self {
        let mut app = App::new();
        app.init_resource::<Inbox>()
            .init_resource::<Effects>()
            .init_resource::<Framebuffer>()
            .init_resource::<GameState>()
            .init_resource::<AudioOut>()
            .add_systems(
                Update,
                (apply_commands, advance_game, direct_audio, render).chain(),
            );
        let mut engine = Self { app };
        engine.update();
        engine
    }

    pub(super) fn command(&mut self, command: EngineCommand) {
        self.app
            .world_mut()
            .resource_mut::<Inbox>()
            .0
            .push_back(command);
    }

    pub fn update(&mut self) {
        self.app.update();
    }

    pub fn frame(&self) -> &[u8] {
        &self.app.world().resource::<Framebuffer>().pixels
    }

    pub(super) fn take_effects(&mut self) -> Vec<EngineEffect> {
        self.app
            .world_mut()
            .resource_mut::<Effects>()
            .0
            .drain(..)
            .collect()
    }

    /// Notes the audio director emitted since the last call, plus the tick.
    pub(super) fn take_audio(&mut self) -> AudioBatch {
        self.app.world_mut().resource_mut::<AudioOut>().drain()
    }

    #[cfg(test)]
    pub(super) fn screen(&self) -> Screen {
        self.app.world().resource::<GameState>().screen
    }
}

impl Default for GameEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct EngineRuntime {
    pub(super) sender: mpsc::Sender<EngineCommand>,
    pub(super) frame: Arc<RwLock<Vec<u8>>>,
    pub(super) audio: Arc<Mutex<AudioQueue>>,
    pub(super) transcript: Arc<Mutex<TranscriptChannel>>,
}

impl EngineRuntime {
    pub fn spawn(
        question_loader: QuestionLoader,
        answered_question_recorder: AnsweredQuestionRecorder,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let frame = Arc::new(RwLock::new(vec![0; FRAME_BYTES]));
        let shared_frame = Arc::clone(&frame);
        let audio = Arc::new(Mutex::new(AudioQueue::default()));
        let shared_audio = Arc::clone(&audio);
        let transcript = Arc::new(Mutex::new(TranscriptChannel::default()));
        let shared_transcript = Arc::clone(&transcript);
        let engine_sender = sender.clone();
        let child = Arc::new(Mutex::new(None));
        let running_child = Arc::clone(&child);

        thread::Builder::new()
            .name("cqa-bevy-engine".into())
            .spawn(move || {
                let mut engine = GameEngine::new();
                let mut next_frame = Instant::now();
                loop {
                    while let Ok(command) = receiver.try_recv() {
                        engine.command(command);
                    }
                    engine.update();
                    for effect in engine.take_effects() {
                        handle_effect(
                            effect,
                            &engine_sender,
                            &running_child,
                            &question_loader,
                            &answered_question_recorder,
                        );
                    }
                    if let Ok(mut target) = shared_frame.write() {
                        target.copy_from_slice(engine.frame());
                    }
                    if let Ok(mut queue) = shared_audio.lock() {
                        queue.append(engine.take_audio());
                    }
                    let (screen, sentences) = engine.transcript_sentences();
                    if let Ok(mut channel) = shared_transcript.lock() {
                        channel.publish(screen, sentences);
                    }
                    next_frame += FRAME_TIME;
                    if let Some(remaining) = next_frame.checked_duration_since(Instant::now()) {
                        thread::sleep(remaining);
                    } else {
                        next_frame = Instant::now();
                    }
                }
            })
            .expect("failed to start Bevy engine thread");

        Self {
            sender,
            frame,
            audio,
            transcript,
        }
    }

    pub fn set_reduced_motion(&self, reduced: bool) -> Result<(), String> {
        self.send(EngineCommand::ReducedMotion(reduced))
    }

    pub fn set_power(&self, powered: bool) -> Result<(), String> {
        self.send(EngineCommand::Power(powered))
    }

    pub fn set_ai_provider(&self, provider: Option<String>) -> Result<(), String> {
        self.send(EngineCommand::AiProvider(provider))
    }

    pub fn finish_boot(&self) -> Result<(), String> {
        self.send(EngineCommand::BootComplete)
    }

    pub fn set_cartridge(&self, cartridge: Option<CartridgeSpec>) -> Result<(), String> {
        self.send(EngineCommand::Cartridge(cartridge))
    }

    pub fn input(&self, button: Button, pressed: bool) -> Result<(), String> {
        self.send(EngineCommand::Input { button, pressed })
    }

    pub fn frame(&self) -> Vec<u8> {
        self.frame
            .read()
            .map(|frame| frame.clone())
            .unwrap_or_else(|_| vec![0; FRAME_BYTES])
    }

    /// Takes every note the engine has emitted since the last drain.
    pub fn drain_audio(&self) -> AudioBatch {
        self.audio
            .lock()
            .map(|mut queue| queue.drain())
            .unwrap_or_default()
    }

    /// The latest screen transcript, when it is newer than `seq`.
    pub fn transcript_since(&self, seq: u64) -> Option<TranscriptUpdate> {
        self.transcript
            .lock()
            .ok()
            .and_then(|channel| channel.since(seq))
    }

    pub(super) fn send(&self, command: EngineCommand) -> Result<(), String> {
        self.sender
            .send(command)
            .map_err(|_| "BEVY ENGINE STOPPED".to_string())
    }
}

pub(super) fn handle_effect(
    effect: EngineEffect,
    sender: &mpsc::Sender<EngineCommand>,
    child: &Arc<Mutex<Option<Child>>>,
    question_loader: &QuestionLoader,
    answered_question_recorder: &AnsweredQuestionRecorder,
) {
    match effect {
        EngineEffect::RunQuest { command, quest } => {
            run_quest(command, quest, sender.clone(), Arc::clone(child))
        }
        EngineEffect::AbortQuest => {
            if let Ok(mut guard) = child.lock() {
                if let Some(process) = guard.as_mut() {
                    // The shell runs the quest's tools as its own children,
                    // and they hold the output pipes: stop all of them.
                    external_tools::kill_process_tree(process);
                }
            }
        }
        EngineEffect::RequestQuestions {
            cartridge_id,
            level,
            count,
            seq,
        } => {
            let sender = sender.clone();
            let loader = Arc::clone(question_loader);
            thread::spawn(move || {
                let result = loader(cartridge_id.clone(), level, count);
                let _ = sender.send(EngineCommand::Questions {
                    cartridge_id,
                    result,
                    seq,
                });
            });
        }
        EngineEffect::RecordAnsweredQuestion {
            cartridge_id,
            evidence,
        } => answered_question_recorder(cartridge_id, ProgressEvent::Answered(evidence)),
        EngineEffect::MarkPeeked {
            cartridge_id,
            question,
        } => answered_question_recorder(cartridge_id, ProgressEvent::Peeked { question }),
    }
}

#[cfg(test)]
#[cfg(any(unix, target_os = "linux"))]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn aborting_a_quest_stops_its_whole_process_tree_and_frees_the_slot() {
        let marker = std::env::temp_dir().join(format!("cqa-quest-abort-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let (sender, receiver) = mpsc::channel();
        let slot = Arc::new(Mutex::new(None));
        let loader: QuestionLoader = Arc::new(|_, _, _| Ok(Vec::new()));
        let recorder: AnsweredQuestionRecorder = Arc::new(|_, _| {});
        let effect =
            |effect: EngineEffect| handle_effect(effect, &sender, &slot, &loader, &recorder);
        let next = || {
            receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("the quest should report")
        };

        // The shell's child holds the output pipes, as cargo or npm would.
        effect(EngineEffect::RunQuest {
            command: format!(
                "(sleep 3; touch '{}') & echo started; wait",
                marker.display()
            ),
            quest: 1,
        });
        while !matches!(next(), EngineCommand::QuestOutput { line, .. } if line == "started") {}
        let aborted = Instant::now();
        effect(EngineEffect::AbortQuest);
        loop {
            if let EngineCommand::QuestDone { success, quest } = next() {
                assert!(!success);
                assert_eq!(quest, 1);
                break;
            }
        }
        // The child sleeps 3 s, so finishing well inside that proves the abort
        // did not wait it out. The bound leaves room for slow CI runners (a
        // macOS runner exceeded half of QUEST_OUTPUT_GRACE); whether the child
        // itself was stopped is checked through its marker below.
        let abort_time = aborted.elapsed();
        assert!(
            abort_time < Duration::from_secs(2),
            "the abort did not wait out the quest's 3 s child ({abort_time:?})"
        );

        effect(EngineEffect::RunQuest {
            command: "echo again".into(),
            quest: 2,
        });
        let mut lines = Vec::new();
        let success = loop {
            match next() {
                EngineCommand::QuestOutput { line, quest, .. } => {
                    assert_eq!(quest, 2);
                    lines.push(line);
                }
                EngineCommand::QuestDone { success, .. } => break success,
                _ => {}
            }
        };
        assert_eq!(lines, ["again"], "the next quest is not refused");
        assert!(success);

        thread::sleep(Duration::from_millis(3_500).saturating_sub(aborted.elapsed()));
        assert!(
            !marker.exists(),
            "the aborted quest's children were stopped"
        );
    }

    /// A helper that escapes the quest's process group keeps the output pipe
    /// open past the grace period, as a Windows helper does once its shell
    /// has exited. Nothing it writes after the quest is done is forwarded.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_surviving_quest_helper_forwards_nothing_after_the_quest_is_done() {
        let (sender, receiver) = mpsc::channel();
        let slot = Arc::new(Mutex::new(None));
        let loader: QuestionLoader = Arc::new(|_, _, _| Ok(Vec::new()));
        let recorder: AnsweredQuestionRecorder = Arc::new(|_, _| {});
        handle_effect(
            EngineEffect::RunQuest {
                command: "setsid sh -c 'sleep 2; echo LEFTOVER; sleep 2' & echo done".into(),
                quest: 7,
            },
            &sender,
            &slot,
            &loader,
            &recorder,
        );
        let started = Instant::now();
        let mut lines = Vec::new();
        loop {
            match receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("the quest should report")
            {
                EngineCommand::QuestOutput { line, quest, .. } => {
                    assert_eq!(quest, 7);
                    lines.push(line);
                }
                EngineCommand::QuestDone { quest, .. } => {
                    assert_eq!(quest, 7);
                    break;
                }
                _ => {}
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "done before the helper writes"
        );
        while let Ok(command) = receiver.recv_timeout(Duration::from_millis(3_000)) {
            if let EngineCommand::QuestOutput { line, .. } = command {
                lines.push(line);
            }
        }
        assert_eq!(lines, ["done"], "the helper's late line is not forwarded");
    }
}
