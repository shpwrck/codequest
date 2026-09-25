use super::*;

pub const QUIZ_QUESTION_COLUMNS: usize = 31;
pub const QUIZ_QUESTION_ROWS: usize = 4;
pub const QUIZ_CHOICE_CHARS: usize = 31;

#[derive(Clone, Debug)]
pub struct QuestSpec {
    pub name: String,
    pub boss: String,
    pub command: String,
}

#[derive(Clone, Debug, Default)]
pub struct QuizQuestion {
    pub question: String,
    pub choices: Vec<String>,
    pub answer: usize,
    /// The conceptual lens this question assesses, when the provider named one.
    pub concept: Option<Concept>,
    /// One rationale per choice (same order as `choices`), or empty for legacy
    /// questions generated before rationales existed.
    pub rationales: Vec<String>,
    /// Whether this question returns because the player previously missed
    /// it, and whether that miss was in this launch or an earlier one.
    pub review: Review,
}

/// Generates a question batch for a cartridge, level, and count. A failure
/// carries a short upper-case reason the waiting Oracle shows the player.
pub type QuestionLoader =
    Arc<dyn Fn(String, u32, usize) -> Result<Vec<QuizQuestion>, String> + Send + Sync + 'static>;
/// Persists a learner-progress event (a committed answer or a Codex reveal)
/// for the cartridge id it is given.
pub type AnsweredQuestionRecorder = Arc<dyn Fn(String, ProgressEvent) + Send + Sync + 'static>;

pub fn quiz_question_fits(question: &str, choices: &[String], answer: usize) -> bool {
    !question.trim().is_empty()
        && wrap_text(question, QUIZ_QUESTION_COLUMNS).len() <= QUIZ_QUESTION_ROWS
        && choices.len() == 4
        && answer < choices.len()
        && choices
            .iter()
            .map(|choice| choice.trim().to_ascii_uppercase())
            .collect::<HashSet<_>>()
            .len()
            == choices.len()
        && choices
            .iter()
            .all(|choice| !choice.trim().is_empty() && choice.chars().count() <= QUIZ_CHOICE_CHARS)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CartridgeMode {
    Quiz,
    Custom,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepositoryProvenance {
    pub authors: Vec<String>,
    pub first_year: Option<u16>,
    pub latest_year: Option<u16>,
    pub copyright: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CartridgeSpec {
    pub id: String,
    pub title: String,
    pub mode: CartridgeMode,
    pub provenance: RepositoryProvenance,
    pub codequest: Option<Box<CodeQuestConfig>>,
    pub machine: Box<SceneMachineDefinition>,
    pub quests: Vec<QuestSpec>,
    pub questions: Vec<QuizQuestion>,
    pub question_batch_ends: Vec<usize>,
    /// The generation level of each batch, parallel to `question_batch_ends`.
    /// Empty when the save did not record levels; the engine then assumes the
    /// batches climb one level each from 1.
    pub question_batch_levels: Vec<u32>,
    /// The player's lesson journal for this cartridge, oldest first.
    pub lessons: Vec<Lesson>,
    /// Per-lens mastery evidence accumulated across launches.
    pub mastery: Mastery,
    /// Committed attempts per normalized question identity, kept across runs
    /// and (for questions still in play) launches, so a question that returns
    /// always rotates its choices past the layout the player last saw.
    pub question_attempts: HashMap<String, u32>,
}

impl CartridgeSpec {
    pub(super) fn mode(&self) -> CartridgeMode {
        match self.codequest.as_ref().map(|config| config.game.game_type) {
            Some(GameType::Quiz) => CartridgeMode::Quiz,
            Some(GameType::Quest) => CartridgeMode::Custom,
            None => self.mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codequest_game_type_controls_the_engine_mode() {
        let mut cartridge = quiz_cartridge();
        cartridge.codequest = Some(Box::new(
            CodeQuestConfig::parse(
                r#"
                    schema_version = 1

                    [game]
                    type = "quest"
                "#,
            )
            .unwrap(),
        ));

        assert_eq!(cartridge.mode(), CartridgeMode::Custom);
    }
}
