use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::scene_machine::{SceneHandler, SceneMachineDefinition, SceneSpec, SceneTransition};

pub const FILE_NAME: &str = "CODEQUEST.toml";
pub const SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeQuestConfig {
    pub schema_version: u32,
    pub game: GameDefinition,
    #[serde(default)]
    pub scenes: Vec<SceneDefinition>,
    #[serde(default)]
    pub mechanics: Vec<MechanicDefinition>,
    #[serde(default)]
    pub art: Vec<ArtRequirement>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GameDefinition {
    #[serde(rename = "type")]
    pub game_type: GameType,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub start_scene: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameType {
    Quiz,
    Quest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SceneDefinition {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub summary: Option<String>,
    #[serde(default)]
    pub mechanics: Vec<String>,
    #[serde(default)]
    pub art: Vec<String>,
    #[serde(default)]
    pub next: Vec<String>,
    pub handler: Option<SceneHandler>,
    #[serde(default)]
    pub transitions: Vec<SceneTransition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MechanicDefinition {
    pub id: String,
    pub summary: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub feedback: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtRequirement {
    pub id: String,
    pub kind: String,
    pub summary: String,
    #[serde(default)]
    pub requirements: Vec<String>,
}

impl CodeQuestConfig {
    pub fn load(repo: &Path) -> Result<Option<Self>, String> {
        let path = repo.join(FILE_NAME);
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!("CANNOT READ {FILE_NAME}: {error}"));
            }
        };
        Self::parse(&source).map(Some)
    }

    pub fn parse(source: &str) -> Result<Self, String> {
        let config: Self =
            toml::from_str(source).map_err(|error| format!("INVALID {FILE_NAME}: {error}"))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        if !matches!(self.schema_version, 1 | SCHEMA_VERSION) {
            return Err(format!(
                "INVALID {FILE_NAME}: unsupported schema_version {}; expected 1 or {SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        validate_optional_text(FILE_NAME, "game.title", self.game.title.as_deref())?;
        validate_optional_text(FILE_NAME, "game.summary", self.game.summary.as_deref())?;

        let scene_ids = unique_ids(
            FILE_NAME,
            "scene",
            self.scenes.iter().map(|scene| &scene.id),
        )?;
        let mechanic_ids = unique_ids(
            FILE_NAME,
            "mechanic",
            self.mechanics.iter().map(|mechanic| &mechanic.id),
        )?;
        let art_ids = unique_ids(FILE_NAME, "art", self.art.iter().map(|art| &art.id))?;

        if self.scenes.is_empty() {
            if let Some(start_scene) = &self.game.start_scene {
                return Err(format!(
                    "INVALID {FILE_NAME}: start_scene `{start_scene}` does not exist"
                ));
            }
        } else {
            let start_scene = self.game.start_scene.as_ref().ok_or_else(|| {
                format!("INVALID {FILE_NAME}: game.start_scene is required when scenes are defined")
            })?;
            validate_reference(FILE_NAME, "start_scene", start_scene, &scene_ids)?;
        }

        for scene in &self.scenes {
            if scene.title.trim().is_empty() {
                return Err(format!(
                    "INVALID {FILE_NAME}: scene `{}` has an empty title",
                    scene.id
                ));
            }
            if scene.kind.trim().is_empty() {
                return Err(format!(
                    "INVALID {FILE_NAME}: scene `{}` has an empty kind",
                    scene.id
                ));
            }
            validate_optional_text(
                FILE_NAME,
                &format!("scene `{}` summary", scene.id),
                scene.summary.as_deref(),
            )?;
            if self.schema_version == 1 {
                if scene.handler.is_some() || !scene.transitions.is_empty() {
                    return Err(format!(
                        "INVALID {FILE_NAME}: scene `{}` runtime fields require schema_version 2",
                        scene.id
                    ));
                }
                for next in &scene.next {
                    validate_reference(FILE_NAME, "scene reference", next, &scene_ids)?;
                }
            } else {
                if !scene.next.is_empty() {
                    return Err(format!(
                        "INVALID {FILE_NAME}: scene `{}` uses v1 `next`; schema_version 2 requires transitions",
                        scene.id
                    ));
                }
                let handler = scene.handler.ok_or_else(|| {
                    format!(
                        "INVALID {FILE_NAME}: scene `{}` requires a handler in schema_version 2",
                        scene.id
                    )
                })?;
                if !handler_supports_game(handler, self.game.game_type) {
                    return Err(format!(
                        "INVALID {FILE_NAME}: scene `{}` handler `{:?}` is not available for `{:?}` games",
                        scene.id, handler, self.game.game_type
                    ));
                }
            }
            for mechanic in &scene.mechanics {
                validate_reference(FILE_NAME, "mechanic reference", mechanic, &mechanic_ids)?;
            }
            for art in &scene.art {
                validate_reference(FILE_NAME, "art reference", art, &art_ids)?;
            }
        }

        for mechanic in &self.mechanics {
            if mechanic.summary.trim().is_empty() {
                return Err(format!(
                    "INVALID {FILE_NAME}: mechanic `{}` has an empty summary",
                    mechanic.id
                ));
            }
        }
        for art in &self.art {
            if art.kind.trim().is_empty() || art.summary.trim().is_empty() {
                return Err(format!(
                    "INVALID {FILE_NAME}: art `{}` requires non-empty kind and summary",
                    art.id
                ));
            }
        }
        self.runtime_machine()?;
        Ok(())
    }

    pub fn runtime_machine(&self) -> Result<Option<SceneMachineDefinition>, String> {
        if self.schema_version == 1 {
            return Ok(None);
        }
        let start_scene = self.game.start_scene.as_deref().ok_or_else(|| {
            format!("INVALID {FILE_NAME}: schema_version 2 requires game.start_scene")
        })?;
        let specs = self
            .scenes
            .iter()
            .map(|scene| {
                let handler = scene.handler.ok_or_else(|| {
                    format!(
                        "INVALID {FILE_NAME}: scene `{}` requires a handler in schema_version 2",
                        scene.id
                    )
                })?;
                Ok(SceneSpec {
                    id: scene.id.clone(),
                    handler,
                    transitions: scene.transitions.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        SceneMachineDefinition::compile(start_scene, specs)
            .map(Some)
            .map_err(|error| format!("INVALID {FILE_NAME}: {error}"))
    }
}

fn handler_supports_game(handler: SceneHandler, game_type: GameType) -> bool {
    use SceneHandler as Handler;

    matches!(
        handler,
        Handler::RepositoryCredits | Handler::OpeningFanfare | Handler::Title
    ) || match game_type {
        GameType::Quiz => matches!(
            handler,
            Handler::QuizMenu
                | Handler::CharacterCreation
                | Handler::Oracle
                | Handler::ConceptQuiz
                | Handler::LevelUp
                | Handler::GameOver
        ),
        GameType::Quest => matches!(
            handler,
            Handler::QuestSelect | Handler::Battle | Handler::Victory | Handler::Defeat
        ),
    }
}

fn validate_optional_text(file_name: &str, field: &str, value: Option<&str>) -> Result<(), String> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        Err(format!("INVALID {file_name}: {field} cannot be empty"))
    } else {
        Ok(())
    }
}

fn unique_ids<'a>(
    file_name: &str,
    item_kind: &str,
    ids: impl Iterator<Item = &'a String>,
) -> Result<HashSet<String>, String> {
    let mut unique = HashSet::new();
    for id in ids {
        if id.trim().is_empty() {
            return Err(format!(
                "INVALID {file_name}: {item_kind} id cannot be empty"
            ));
        }
        if !unique.insert(id.clone()) {
            return Err(format!(
                "INVALID {file_name}: duplicate {item_kind} id `{id}`"
            ));
        }
    }
    Ok(unique)
}

fn validate_reference(
    file_name: &str,
    reference_kind: &str,
    id: &str,
    known_ids: &HashSet<String>,
) -> Result<(), String> {
    if known_ids.contains(id) {
        Ok(())
    } else {
        Err(format!(
            "INVALID {file_name}: {reference_kind} `{id}` does not exist"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_machine::{SceneEvent, SceneMachine, SceneSignal};

    #[test]
    fn schema_v2_compiles_an_executable_scene_machine() {
        let config = CodeQuestConfig::parse(
            r#"
                schema_version = 2

                [game]
                type = "quiz"
                start_scene = "copyright"

                [[scenes]]
                id = "copyright"
                title = "Repository Copyright"
                kind = "credits"
                handler = "repository-credits"

                [[scenes.transitions]]
                signal = "continue"
                target = "title"
                after_ticks = 2

                [[scenes.transitions]]
                signal = "elapsed"
                target = "title"
                after_ticks = 3

                [[scenes]]
                id = "title"
                title = "Title"
                kind = "title"
                handler = "title"
            "#,
        )
        .unwrap();

        let definition = config.runtime_machine().unwrap().unwrap();
        let mut machine = SceneMachine::new(definition.clone());
        assert_eq!(machine.current_scene(), "copyright");
        assert_eq!(
            machine.handle(SceneEvent::Signal(SceneSignal::Continue)),
            None
        );
        assert_eq!(machine.handle(SceneEvent::Tick), None);
        assert_eq!(machine.handle(SceneEvent::Tick), None);
        assert_eq!(
            machine
                .handle(SceneEvent::Signal(SceneSignal::Continue))
                .unwrap()
                .target,
            "title"
        );

        let mut automatic = SceneMachine::new(definition);
        assert_eq!(automatic.handle(SceneEvent::Tick), None);
        assert_eq!(automatic.handle(SceneEvent::Tick), None);
        assert_eq!(automatic.handle(SceneEvent::Tick).unwrap().target, "title");
    }

    #[test]
    fn schema_v2_rejects_handlers_from_the_wrong_game_family() {
        let error = CodeQuestConfig::parse(
            r#"
                schema_version = 2

                [game]
                type = "quiz"
                start_scene = "battle"

                [[scenes]]
                id = "battle"
                title = "Battle"
                kind = "challenge"
                handler = "battle"
            "#,
        )
        .expect_err("quiz cartridges cannot invoke quest-only runtime systems");

        assert!(error.contains("handler `Battle` is not available for `Quiz` games"));
    }

    #[test]
    fn repository_manifest_defines_the_shipped_quiz_storyboard() {
        let repository_manifest = include_str!("../../CODEQUEST.toml");
        let documented_example = include_str!("../../docs/examples/CODEQUEST.toml");
        assert_eq!(
            repository_manifest, documented_example,
            "the dogfood manifest and documented example must remain synchronized"
        );
        let config = CodeQuestConfig::parse(repository_manifest)
            .expect("the repository's dogfood manifest should remain valid");

        assert_eq!(config.schema_version, 2);
        assert_eq!(config.game.game_type, GameType::Quiz);
        assert_eq!(config.game.start_scene.as_deref(), Some("copyright"));
        let copyright = config
            .scenes
            .iter()
            .find(|scene| scene.id == "copyright")
            .expect("the storyboard should begin with repository provenance");
        assert_eq!(copyright.handler, Some(SceneHandler::RepositoryCredits));
        assert!(copyright.transitions.iter().all(|transition| {
            transition.target == "opening-fanfare" && transition.after_ticks.is_some()
        }));
        let fanfare = config
            .scenes
            .iter()
            .find(|scene| scene.id == "opening-fanfare")
            .expect("the storyboard should include an opening fanfare");
        assert_eq!(fanfare.handler, Some(SceneHandler::OpeningFanfare));
        assert!(fanfare
            .transitions
            .iter()
            .all(|transition| transition.target == "title"));
        assert!(config.scenes.iter().any(|scene| scene.id == "quiz"));
        assert!(config.runtime_machine().unwrap().is_some());
    }

    #[test]
    fn unknown_fields_are_rejected_instead_of_silently_ignored() {
        let error = CodeQuestConfig::parse(
            r#"
                schema_version = 1

                [game]
                type = "quiz"
                typo = "this should not be accepted"
            "#,
        )
        .expect_err("unknown fields hide authoring mistakes");

        assert!(error.contains("unknown field `typo`"));
    }

    #[test]
    fn storyboard_references_must_resolve() {
        let error = CodeQuestConfig::parse(
            r#"
                schema_version = 1

                [game]
                type = "quiz"
                start_scene = "missing"

                [[scenes]]
                id = "title"
                title = "Title"
                kind = "title"
                next = ["also-missing"]
            "#,
        )
        .expect_err("broken scene links should fail at cartridge load time");

        assert!(error.contains("start_scene `missing` does not exist"));
    }

    #[test]
    fn unsupported_schema_versions_are_rejected() {
        let error = CodeQuestConfig::parse(
            r#"
                schema_version = 3

                [game]
                type = "quiz"
            "#,
        )
        .expect_err("new schema versions need explicit engine support");

        assert!(error.contains("unsupported schema_version 3"));
    }

    #[test]
    fn present_optional_text_cannot_be_blank() {
        let error = CodeQuestConfig::parse(
            r#"
                schema_version = 1

                [game]
                type = "quiz"
                summary = "  "
            "#,
        )
        .expect_err("blank design text is not useful metadata");

        assert!(error.contains("game.summary cannot be empty"));
    }
}
