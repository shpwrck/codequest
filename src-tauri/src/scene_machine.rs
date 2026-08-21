use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneHandler {
    RepositoryCredits,
    OpeningFanfare,
    Title,
    QuizMenu,
    CharacterCreation,
    Oracle,
    ConceptQuiz,
    LevelUp,
    GameOver,
    QuestSelect,
    Battle,
    Victory,
    Defeat,
}

impl SceneHandler {
    fn supports(self, signal: SceneSignal) -> bool {
        use SceneHandler as Handler;
        use SceneSignal as Signal;

        match self {
            Handler::RepositoryCredits | Handler::OpeningFanfare => {
                matches!(signal, Signal::Continue | Signal::Elapsed)
            }
            Handler::Title => signal == Signal::Continue,
            Handler::QuizMenu => matches!(signal, Signal::NewRun | Signal::Back),
            Handler::CharacterCreation => matches!(signal, Signal::HeroReady | Signal::Back),
            Handler::Oracle => matches!(signal, Signal::QuestionsReady | Signal::Back),
            Handler::ConceptQuiz => matches!(
                signal,
                Signal::NeedsQuestion | Signal::BatchComplete | Signal::HeartsEmpty | Signal::Back
            ),
            Handler::LevelUp => {
                matches!(signal, Signal::QuestionsReady | Signal::NeedsQuestion)
            }
            Handler::GameOver => signal == Signal::Replay,
            Handler::QuestSelect => matches!(signal, Signal::QuestSelected | Signal::Back),
            Handler::Battle => matches!(signal, Signal::Victory | Signal::Defeat),
            Handler::Victory | Handler::Defeat => signal == Signal::Continue,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneSignal {
    Elapsed,
    Continue,
    Back,
    NewRun,
    HeroReady,
    QuestionsReady,
    NeedsQuestion,
    BatchComplete,
    HeartsEmpty,
    Replay,
    QuestSelected,
    Victory,
    Defeat,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SceneTransition {
    pub signal: SceneSignal,
    pub target: String,
    pub after_ticks: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneSpec {
    pub id: String,
    pub handler: SceneHandler,
    pub transitions: Vec<SceneTransition>,
}

#[derive(Clone, Debug)]
struct CompiledScene {
    handler: SceneHandler,
    transitions: Vec<SceneTransition>,
}

#[derive(Clone, Debug)]
pub struct SceneMachineDefinition {
    start_scene: String,
    scenes: HashMap<String, CompiledScene>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneMachineTemplate {
    Quiz,
    Quest,
}

impl SceneMachineDefinition {
    pub fn compile(start_scene: &str, specs: Vec<SceneSpec>) -> Result<Self, String> {
        let mut scenes = HashMap::new();
        for spec in specs {
            if spec.id.trim().is_empty() {
                return Err("scene-machine scene id cannot be empty".into());
            }
            let id = spec.id;
            let scene = CompiledScene {
                handler: spec.handler,
                transitions: spec.transitions,
            };
            if scenes.insert(id.clone(), scene).is_some() {
                return Err(format!("scene-machine has duplicate scene `{id}`"));
            }
        }

        if !scenes.contains_key(start_scene) {
            return Err(format!(
                "scene-machine start scene `{start_scene}` does not exist"
            ));
        }

        for (id, scene) in &scenes {
            let mut signals = HashSet::new();
            for transition in &scene.transitions {
                if !scenes.contains_key(&transition.target) {
                    return Err(format!(
                        "scene-machine transition from `{id}` targets missing scene `{}`",
                        transition.target
                    ));
                }
                if !scene.handler.supports(transition.signal) {
                    return Err(format!(
                        "scene `{id}` handler `{:?}` cannot emit signal `{:?}`",
                        scene.handler, transition.signal
                    ));
                }
                if transition.signal == SceneSignal::Elapsed && transition.after_ticks.is_none() {
                    return Err(format!(
                        "scene `{id}` elapsed transition requires after_ticks"
                    ));
                }
                if !signals.insert(transition.signal) {
                    return Err(format!(
                        "scene `{id}` has ambiguous `{:?}` transitions",
                        transition.signal
                    ));
                }
            }
        }

        let mut reachable = HashSet::new();
        let mut pending = VecDeque::from([start_scene.to_string()]);
        while let Some(id) = pending.pop_front() {
            if !reachable.insert(id.clone()) {
                continue;
            }
            for transition in &scenes[&id].transitions {
                pending.push_back(transition.target.clone());
            }
        }
        let mut unreachable: Vec<_> = scenes
            .keys()
            .filter(|id| !reachable.contains(*id))
            .cloned()
            .collect();
        if !unreachable.is_empty() {
            unreachable.sort();
            return Err(format!(
                "scene-machine has unreachable scenes: {}",
                unreachable.join(", ")
            ));
        }

        Ok(Self {
            start_scene: start_scene.to_string(),
            scenes,
        })
    }

    pub fn template(template: SceneMachineTemplate) -> Self {
        use SceneHandler as Handler;
        use SceneSignal as Signal;

        fn transition(
            signal: SceneSignal,
            target: &str,
            after_ticks: Option<u64>,
        ) -> SceneTransition {
            SceneTransition {
                signal,
                target: target.into(),
                after_ticks,
            }
        }

        fn scene(id: &str, handler: SceneHandler, transitions: Vec<SceneTransition>) -> SceneSpec {
            SceneSpec {
                id: id.into(),
                handler,
                transitions,
            }
        }

        let mut scenes = vec![
            scene(
                "copyright",
                Handler::RepositoryCredits,
                vec![
                    transition(Signal::Continue, "opening-fanfare", Some(60)),
                    transition(Signal::Elapsed, "opening-fanfare", Some(180)),
                ],
            ),
            scene(
                "opening-fanfare",
                Handler::OpeningFanfare,
                vec![
                    transition(Signal::Continue, "title", Some(90)),
                    transition(Signal::Elapsed, "title", Some(330)),
                ],
            ),
        ];

        match template {
            SceneMachineTemplate::Quiz => scenes.extend([
                scene(
                    "title",
                    Handler::Title,
                    vec![transition(Signal::Continue, "quiz-menu", None)],
                ),
                scene(
                    "quiz-menu",
                    Handler::QuizMenu,
                    vec![
                        transition(Signal::NewRun, "character-creation", None),
                        transition(Signal::Back, "title", None),
                    ],
                ),
                scene(
                    "character-creation",
                    Handler::CharacterCreation,
                    vec![
                        transition(Signal::HeroReady, "oracle", None),
                        transition(Signal::Back, "quiz-menu", None),
                    ],
                ),
                scene(
                    "oracle",
                    Handler::Oracle,
                    vec![
                        transition(Signal::QuestionsReady, "quiz", None),
                        transition(Signal::Back, "quiz-menu", None),
                    ],
                ),
                scene(
                    "quiz",
                    Handler::ConceptQuiz,
                    vec![
                        transition(Signal::NeedsQuestion, "oracle", None),
                        transition(Signal::BatchComplete, "level-up", None),
                        transition(Signal::HeartsEmpty, "game-over", None),
                        transition(Signal::Back, "quiz-menu", None),
                    ],
                ),
                scene(
                    "level-up",
                    Handler::LevelUp,
                    vec![
                        transition(Signal::QuestionsReady, "quiz", None),
                        transition(Signal::NeedsQuestion, "oracle", None),
                    ],
                ),
                scene(
                    "game-over",
                    Handler::GameOver,
                    vec![transition(Signal::Replay, "quiz-menu", None)],
                ),
            ]),
            SceneMachineTemplate::Quest => scenes.extend([
                scene(
                    "title",
                    Handler::Title,
                    vec![transition(Signal::Continue, "quest-select", None)],
                ),
                scene(
                    "quest-select",
                    Handler::QuestSelect,
                    vec![
                        transition(Signal::QuestSelected, "battle", None),
                        transition(Signal::Back, "title", None),
                    ],
                ),
                scene(
                    "battle",
                    Handler::Battle,
                    vec![
                        transition(Signal::Victory, "victory", None),
                        transition(Signal::Defeat, "defeat", None),
                    ],
                ),
                scene(
                    "victory",
                    Handler::Victory,
                    vec![transition(Signal::Continue, "quest-select", None)],
                ),
                scene(
                    "defeat",
                    Handler::Defeat,
                    vec![transition(Signal::Continue, "quest-select", None)],
                ),
            ]),
        }

        Self::compile("copyright", scenes).expect("built-in scene-machine template must be valid")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneEvent {
    Tick,
    Signal(SceneSignal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneChange {
    pub source: String,
    pub target: String,
    pub handler: SceneHandler,
}

#[derive(Clone, Debug)]
pub struct SceneMachine {
    definition: SceneMachineDefinition,
    current: String,
    elapsed_ticks: u64,
}

impl SceneMachine {
    pub fn new(definition: SceneMachineDefinition) -> Self {
        let current = definition.start_scene.clone();
        Self {
            definition,
            current,
            elapsed_ticks: 0,
        }
    }

    pub fn reset(&mut self) {
        self.current.clone_from(&self.definition.start_scene);
        self.elapsed_ticks = 0;
    }

    pub fn current_scene(&self) -> &str {
        &self.current
    }

    pub fn current_handler(&self) -> SceneHandler {
        self.definition.scenes[&self.current].handler
    }

    pub fn can_handle(&self, signal: SceneSignal) -> bool {
        self.definition.scenes[&self.current]
            .transitions
            .iter()
            .any(|transition| {
                transition.signal == signal
                    && self.elapsed_ticks >= transition.after_ticks.unwrap_or_default()
            })
    }

    pub fn handle(&mut self, event: SceneEvent) -> Option<SceneChange> {
        let signal = match event {
            SceneEvent::Tick => {
                self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
                SceneSignal::Elapsed
            }
            SceneEvent::Signal(signal) => signal,
        };
        let scene = &self.definition.scenes[&self.current];
        let transition = scene.transitions.iter().find(|transition| {
            transition.signal == signal
                && self.elapsed_ticks >= transition.after_ticks.unwrap_or_default()
        })?;
        let source = std::mem::replace(&mut self.current, transition.target.clone());
        self.elapsed_ticks = 0;
        Some(SceneChange {
            source,
            target: self.current.clone(),
            handler: self.current_handler(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(id: &str, handler: SceneHandler, transitions: Vec<SceneTransition>) -> SceneSpec {
        SceneSpec {
            id: id.into(),
            handler,
            transitions,
        }
    }

    fn route(signal: SceneSignal, target: &str) -> SceneTransition {
        SceneTransition {
            signal,
            target: target.into(),
            after_ticks: None,
        }
    }

    #[test]
    fn compile_rejects_signals_the_handler_cannot_emit() {
        let error = SceneMachineDefinition::compile(
            "title",
            vec![scene(
                "title",
                SceneHandler::Title,
                vec![route(SceneSignal::Back, "title")],
            )],
        )
        .unwrap_err();

        assert!(error.contains("cannot emit signal"));
    }

    #[test]
    fn compile_rejects_unreachable_scenes() {
        let error = SceneMachineDefinition::compile(
            "title",
            vec![
                scene("title", SceneHandler::Title, vec![]),
                scene("orphan", SceneHandler::Title, vec![]),
            ],
        )
        .unwrap_err();

        assert!(error.contains("unreachable scenes: orphan"));
    }
}
