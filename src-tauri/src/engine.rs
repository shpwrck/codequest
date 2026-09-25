use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::codequest::{CodeQuestConfig, GameType, VisualTemplate};
use crate::external_tools;
use crate::font5x7::{glyph, GLYPH_ADVANCE, GLYPH_WIDTH, LINE_HEIGHT};
use crate::learning::{
    self, AnswerEvidence, Concept, LensRecord, Lesson, Mastery, RATIONALE_COLUMNS, RATIONALE_ROWS,
};
use crate::scene_machine::{
    SceneEvent, SceneHandler, SceneMachine, SceneMachineDefinition, SceneSignal,
};

pub const WIDTH: usize = 240;
pub const HEIGHT: usize = 160;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT * 4;
const NATIVE_RGB_BYTES: usize = WIDTH * HEIGHT * 3;
const HERO_SPRITE_WIDTH: usize = 24;
const HERO_SPRITE_HEIGHT: usize = 36;
const HERO_SPRITE_BYTES: usize = HERO_SPRITE_WIDTH * HERO_SPRITE_HEIGHT * 4;
const HERO_PORTRAIT_SIZE: usize = 24;
const HERO_PORTRAIT_BYTES: usize = HERO_PORTRAIT_SIZE * HERO_PORTRAIT_SIZE * 4;
const DROP_SPRITE_SIZE: usize = 16;
const DROP_SPRITE_BYTES: usize = DROP_SPRITE_SIZE * DROP_SPRITE_SIZE * 4;
pub const QUIZ_QUESTION_COLUMNS: usize = 31;
pub const QUIZ_QUESTION_ROWS: usize = 4;
pub const QUIZ_CHOICE_CHARS: usize = 31;
const QUESTION_BATCH_SIZE: usize = 6;
const FRAME_TIME: Duration = Duration::from_nanos(16_666_667);
const ORACLE_HERO_MIN_X: i32 = 8;
const ORACLE_HERO_MAX_X: i32 = 210;
const ORACLE_HERO_SPEED: i32 = 2;
const ORACLE_DROP_INTERVAL: u64 = 30;
const ORACLE_COLLISION_Y: i32 = 100;
const QUIZ_FEEDBACK_TICKS: u16 = 45;
/// Ticks after a first B in which a second B leaves an active question.
const QUIZ_LEAVE_CONFIRM_TICKS: u16 = 90;
/// Questions answered between a miss and its review copy within one batch.
const RETRY_GAP: usize = 3;
const LEVEL_UP_HOLD_TICKS: u64 = 60;
const SCORE_RUNE_THRESHOLDS: [u32; 3] = [300, 900, 1_800];
const DATA_CHARGE_THRESHOLDS: [u32; 3] = [3, 6, 9];
const BUG_BREACH_THRESHOLDS: [u32; 3] = [1, 3, 5];

const INK: Color = Color::rgb(26, 28, 44);
const NAVY: Color = Color::rgb(41, 54, 111);
const ROYAL: Color = Color::rgb(59, 93, 201);
const SKY: Color = Color::rgb(65, 166, 246);
const PARCH: Color = Color::rgb(244, 244, 244);
const MIST: Color = Color::rgb(148, 176, 194);
const GOLD: Color = Color::rgb(255, 205, 117);
const GREEN: Color = Color::rgb(56, 183, 100);
const RED: Color = Color::rgb(225, 75, 95);
const PLUM: Color = Color::rgb(93, 39, 93);
const CRAB: Color = Color::rgb(206, 142, 107);
const VOID: Color = Color::rgb(7, 10, 24);
const INDIGO: Color = Color::rgb(22, 29, 66);
const CYAN: Color = Color::rgb(67, 224, 244);
const CYAN_DIM: Color = Color::rgb(47, 146, 174);
const AMBER: Color = Color::rgb(247, 183, 72);
const VIOLET: Color = Color::rgb(105, 58, 151);
const MAGENTA: Color = Color::rgb(213, 91, 151);
const ASH: Color = Color::rgb(68, 78, 105);
const HERO_NAMES: [&str; 6] = ["SUDO", "GREP", "VIM", "FORK", "ASYNC", "PATCH"];
const HERO_CLASSES: [&str; 6] = [
    "CODE KNIGHT",
    "BUG MAGE",
    "PIPE MONK",
    "MERGE PALADIN",
    "LINT RANGER",
    "SHELL DRUID",
];
const HERO_STYLES: [&str; 5] = ["EMBER", "OCEAN", "FOREST", "GOLD", "VOID"];
const HERO_STYLE_COLORS: [Color; 5] = [RED, SKY, GREEN, GOLD, PLUM];

const ORACLE_CHRONICLE: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/chronicle.rgb");
const ORACLE_AWAKENING_SOURCE: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../assets/oracle/awakening-source.rgb");
const ORACLE_AWAKENING_SIGNAL: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../assets/oracle/awakening-signal.rgb");
const ORACLE_AWAKENING_ARCHIVE: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../assets/oracle/awakening-archive.rgb");
const ORACLE_AWAKENING_CONVERGENCE: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../assets/oracle/awakening-convergence.rgb");
const ORACLE_AWAKENING: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/awakening.rgb");
const ORACLE_GATEWAY: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/gateway.rgb");
const ORACLE_ATELIER: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/atelier.rgb");
const ORACLE_SANCTUM: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/sanctum.rgb");
const ORACLE_TRIAL: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/trial.rgb");
const ORACLE_ASCENSION: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/ascension.rgb");
const ORACLE_AFTERMATH: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/aftermath.rgb");

const ORACLE_HEROES: [&[u8; HERO_SPRITE_BYTES]; 5] = [
    include_bytes!("../assets/oracle/hero-magenta.rgba"),
    include_bytes!("../assets/oracle/hero-cyan.rgba"),
    include_bytes!("../assets/oracle/hero-emerald.rgba"),
    include_bytes!("../assets/oracle/hero-amber.rgba"),
    include_bytes!("../assets/oracle/hero-violet.rgba"),
];

const ORACLE_PORTRAITS: [&[u8; HERO_PORTRAIT_BYTES]; 5] = [
    include_bytes!("../assets/oracle/portrait-magenta.rgba"),
    include_bytes!("../assets/oracle/portrait-cyan.rgba"),
    include_bytes!("../assets/oracle/portrait-emerald.rgba"),
    include_bytes!("../assets/oracle/portrait-amber.rgba"),
    include_bytes!("../assets/oracle/portrait-violet.rgba"),
];

const ORACLE_DATA_DROP: &[u8; DROP_SPRITE_BYTES] =
    include_bytes!("../assets/oracle/drop-data.rgba");
const ORACLE_BUG_DROP: &[u8; DROP_SPRITE_BYTES] = include_bytes!("../assets/oracle/drop-bug.rgba");

#[derive(Clone, Copy)]
struct Color(u8, u8, u8);

impl Color {
    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(r, g, b)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UiBox {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

const CHRONICLE_HEADER_BOX: UiBox = UiBox {
    x: 68,
    y: 39,
    width: 104,
    height: 7,
};
const CHRONICLE_TITLE_BOX: UiBox = UiBox {
    x: 62,
    y: 49,
    width: 116,
    height: 16,
};
const CHRONICLE_COPYRIGHT_BOX: UiBox = UiBox {
    x: 62,
    y: 67,
    width: 116,
    height: 16,
};
const CHRONICLE_AUTHORS_LABEL_BOX: UiBox = UiBox {
    x: 62,
    y: 85,
    width: 116,
    height: 7,
};
const CHRONICLE_AUTHORS_BOX: UiBox = UiBox {
    x: 62,
    y: 94,
    width: 116,
    height: 23,
};
const GATEWAY_TITLE_TOP_BOX: UiBox = UiBox {
    x: 56,
    y: 49,
    width: 128,
    height: 14,
};
const GATEWAY_TITLE_BOTTOM_BOX: UiBox = UiBox {
    x: 63,
    y: 67,
    width: 114,
    height: 7,
};
const GATEWAY_PROMPT_BOX: UiBox = UiBox {
    x: 68,
    y: 98,
    width: 104,
    height: 7,
};
const GATEWAY_SIGNATURE_BOX: UiBox = UiBox {
    x: 68,
    y: 123,
    width: 104,
    height: 7,
};
const GATEWAY_MENU_HEADING_BOX: UiBox = UiBox {
    x: 54,
    y: 49,
    width: 132,
    height: 7,
};
const GATEWAY_MENU_SUBTITLE_BOX: UiBox = UiBox {
    x: 54,
    y: 62,
    width: 132,
    height: 7,
};
const GATEWAY_MENU_OPTION_BOXES: [UiBox; 2] = [
    UiBox {
        x: 59,
        y: 92,
        width: 121,
        height: 20,
    },
    UiBox {
        x: 59,
        y: 119,
        width: 121,
        height: 20,
    },
];
const GATEWAY_MENU_OPTION_TEXT_BOXES: [UiBox; 2] = [
    UiBox {
        x: 66,
        y: 94,
        width: 108,
        height: 14,
    },
    UiBox {
        x: 66,
        y: 121,
        width: 108,
        height: 14,
    },
];
const MENU_FOOTER_BOX: UiBox = UiBox {
    x: 39,
    y: 144,
    width: 162,
    height: 16,
};
const ATELIER_HEADER_BOX: UiBox = UiBox {
    x: 6,
    y: 4,
    width: 106,
    height: 16,
};
const ATELIER_ROW_BOXES: [UiBox; 3] = [
    UiBox {
        x: 116,
        y: 17,
        width: 98,
        height: 29,
    },
    UiBox {
        x: 116,
        y: 49,
        width: 98,
        height: 29,
    },
    UiBox {
        x: 116,
        y: 81,
        width: 98,
        height: 29,
    },
];

const fn atelier_label_box(row_box: UiBox) -> UiBox {
    UiBox {
        x: row_box.x + 4,
        y: row_box.y + 5,
        width: row_box.width - 8,
        height: 7,
    }
}

const fn atelier_value_box(row_box: UiBox) -> UiBox {
    UiBox {
        x: row_box.x + 4,
        y: row_box.y + 15,
        width: row_box.width - 8,
        height: 9,
    }
}

const ATELIER_BIND_BOX: UiBox = UiBox {
    x: 137,
    y: 112,
    width: 55,
    height: 21,
};
const ATELIER_BIND_TEXT_BOX: UiBox = UiBox {
    x: 137,
    y: 113,
    width: 55,
    height: 12,
};
const ATELIER_HERO_X: i32 = 32;
const ATELIER_HERO_Y: i32 = 38;
const ATELIER_HERO_SCALE: i32 = 2;
const TRIAL_CHOICE_TEXT_X: i32 = 39;
const TRIAL_WARD_X: i32 = 126;
const TRIAL_STREAK_X: i32 = 180;
const TRIAL_SCORE_RUNES_X: i32 = 194;
const TRIAL_SCORE_X: i32 = 215;
/// The composed lesson panel that replaces the trial plate's four choice
/// frames after a commitment. It spans the frames plus the free margin beside
/// them so a full rationale row keeps visible padding inside the border.
const TRIAL_LESSON_BOX: UiBox = UiBox {
    x: 20,
    y: 69,
    width: 210,
    height: 89,
};
/// The legacy quiz's lesson panel over its choice rows.
const QUIZ_LESSON_BOX: UiBox = UiBox {
    x: 5,
    y: 68,
    width: 230,
    height: 80,
};
/// Extra pixels between the misconception block and the answer block.
const LESSON_BLOCK_GAP: i32 = 4;
const ASCENSION_TITLE_BOX: UiBox = UiBox {
    x: 47,
    y: 68,
    width: 146,
    height: 34,
};
const ASCENSION_LEVEL_BOX: UiBox = UiBox {
    x: 22,
    y: 117,
    width: 70,
    height: 16,
};
const ASCENSION_BATCH_BOX: UiBox = UiBox {
    x: 158,
    y: 117,
    width: 75,
    height: 16,
};
const AFTERMATH_CONTENT_BOX: UiBox = UiBox {
    x: 141,
    y: 18,
    width: 85,
    height: 116,
};
const CODEX_HEADING_BOX: UiBox = UiBox {
    x: 62,
    y: 40,
    width: 116,
    height: 7,
};
const CODEX_LENS_ROW_Y: i32 = 54;
const CODEX_LENS_ROW_PITCH: i32 = 11;
const CODEX_LENS_LABEL_X: i32 = 69;
const CODEX_LENS_RUNES_X: i32 = 133;
const CODEX_LENS_PENDING_X: i32 = 159;
const CODEX_TOTALS_BOX: UiBox = UiBox {
    x: 42,
    y: 122,
    width: 156,
    height: 17,
};
const CODEX_PROMPT_BOX: UiBox = UiBox {
    x: 42,
    y: 141,
    width: 156,
    height: 16,
};
const CODEX_LESSON_HEADER_BOX: UiBox = UiBox {
    x: 37,
    y: 2,
    width: 203,
    height: 28,
};
const CODEX_QUESTION_BOX: UiBox = UiBox {
    x: 17,
    y: 31,
    width: 197,
    height: 38,
};
/// Lesson panels start clear of the trial plate's portrait frame and end
/// before its brazier (question) or pillar (answer and rationale).
const CODEX_ANSWER_BOX: UiBox = UiBox {
    x: 17,
    y: 71,
    width: 212,
    height: 15,
};
const CODEX_RATIONALE_BOX: UiBox = UiBox {
    x: 17,
    y: 88,
    width: 212,
    height: 66,
};
const CODEX_TEXT_X: i32 = 23;
const CODEX_ANSWER_TEXT_X: i32 = 31;
const CODEX_QUESTION_Y: i32 = 35;
const CODEX_ANSWER_Y: i32 = 75;
const CODEX_WHY_Y: i32 = 103;
const CODEX_RATIONALE_Y: i32 = 116;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PresentationTier {
    Initiate,
    Adept,
    OracleBound,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum InsightStage {
    Unlit,
    RuneOne,
    RuneTwo,
    RuneThree,
}

impl InsightStage {
    fn from_score(score: u32) -> Self {
        match threshold_stage(score, &SCORE_RUNE_THRESHOLDS) {
            0 => Self::Unlit,
            1 => Self::RuneOne,
            2 => Self::RuneTwo,
            _ => Self::RuneThree,
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    fn label(self) -> &'static str {
        match self {
            Self::Unlit => "UNLIT",
            Self::RuneOne => "I",
            Self::RuneTwo => "II",
            Self::RuneThree => "III",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Unlit => MIST,
            Self::RuneOne => CYAN,
            Self::RuneTwo => AMBER,
            Self::RuneThree => MAGENTA,
        }
    }
}

fn threshold_stage(value: u32, thresholds: &[u32]) -> usize {
    thresholds.partition_point(|threshold| value >= *threshold)
}

fn streak_multiplier(streak: u32) -> u32 {
    match streak {
        0..=2 => 1,
        3..=5 => 2,
        _ => 3,
    }
}

fn score_award_for_streak(streak: u32) -> u32 {
    100 * streak_multiplier(streak)
}

impl PresentationTier {
    fn from_level(level: u32) -> Self {
        match level {
            0 | 1 => Self::Initiate,
            2 | 3 => Self::Adept,
            _ => Self::OracleBound,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Initiate => "INITIATE",
            Self::Adept => "ADEPT",
            Self::OracleBound => "ORACLE-BOUND",
        }
    }
}

#[derive(Clone, Debug)]
pub struct QuestSpec {
    pub name: String,
    pub boss: String,
    pub command: String,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // SCAFFOLD: remove once lesson review reads rationales.
pub struct QuizQuestion {
    pub question: String,
    pub choices: Vec<String>,
    pub answer: usize,
    /// The conceptual lens this question assesses, when the provider named one.
    pub concept: Option<Concept>,
    /// One rationale per choice (same order as `choices`), or empty for legacy
    /// questions generated before rationales existed.
    pub rationales: Vec<String>,
    /// True when this question returns because the player previously missed it.
    pub review: bool,
}

pub type QuestionLoader =
    Arc<dyn Fn(String, u32, usize) -> Vec<QuizQuestion> + Send + Sync + 'static>;
pub type AnsweredQuestionRecorder = Arc<dyn Fn(String, AnswerEvidence) + Send + Sync + 'static>;

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
#[allow(dead_code)] // SCAFFOLD: remove once the Codex reads lessons and mastery.
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
    /// The player's lesson journal for this cartridge, oldest first.
    pub lessons: Vec<Lesson>,
    /// Per-lens mastery evidence accumulated across launches.
    pub mastery: Mastery,
}

impl CartridgeSpec {
    fn mode(&self) -> CartridgeMode {
        match self.codequest.as_ref().map(|config| config.game.game_type) {
            Some(GameType::Quiz) => CartridgeMode::Quiz,
            Some(GameType::Quest) => CartridgeMode::Custom,
            None => self.mode,
        }
    }
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Screen {
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

#[derive(Clone, Debug)]
enum EngineCommand {
    Power(bool),
    AiProvider(Option<String>),
    BootComplete,
    Cartridge(Option<CartridgeSpec>),
    Questions {
        cartridge_id: String,
        questions: Vec<QuizQuestion>,
    },
    Input {
        button: Button,
        pressed: bool,
    },
    QuestOutput {
        line: String,
        stderr: bool,
    },
    QuestDone {
        success: bool,
    },
}

#[derive(Clone, Debug)]
enum EngineEffect {
    RunQuest(String),
    AbortQuest,
    RequestQuestions {
        cartridge_id: String,
        level: u32,
        count: usize,
    },
    RecordAnsweredQuestion {
        cartridge_id: String,
        evidence: AnswerEvidence,
    },
}

#[derive(Resource, Default)]
struct Inbox(VecDeque<EngineCommand>);

#[derive(Resource, Default)]
struct Effects(VecDeque<EngineEffect>);

#[derive(Resource)]
struct Framebuffer {
    pixels: Vec<u8>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self {
            pixels: vec![0; FRAME_BYTES],
        }
    }
}

impl Framebuffer {
    fn clear(&mut self, color: Color) {
        for pixel in self.pixels.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&[color.0, color.1, color.2, 255]);
        }
    }

    fn blit_rgb(&mut self, rgb: &[u8; NATIVE_RGB_BYTES]) {
        for (source, destination) in rgb
            .as_chunks::<3>()
            .0
            .iter()
            .zip(self.pixels.as_chunks_mut::<4>().0.iter_mut())
        {
            destination.copy_from_slice(&[source[0], source[1], source[2], 255]);
        }
    }

    fn blit_rgb_graded(&mut self, rgb: &[u8; NATIVE_RGB_BYTES], base: u16, cyan: u16, gold: u16) {
        for (source, destination) in rgb
            .as_chunks::<3>()
            .0
            .iter()
            .zip(self.pixels.as_chunks_mut::<4>().0.iter_mut())
        {
            let [red, green, blue] = [source[0] as u16, source[1] as u16, source[2] as u16];
            let scale = if blue > red.saturating_add(12) && green > red {
                cyan
            } else if red > blue.saturating_add(14) && green > blue {
                gold
            } else {
                base
            };
            destination.copy_from_slice(&[
                (red * scale / 255) as u8,
                (green * scale / 255) as u8,
                (blue * scale / 255) as u8,
                255,
            ]);
        }
    }

    fn blit_awakening(&mut self, ticks: u64) {
        let cyan_strength = 38 + ticks.saturating_sub(36).min(108) as u16 * 190 / 108;
        let gold_strength = 38 + ticks.saturating_sub(112).min(108) as u16 * 197 / 108;
        let center_strength = 38 + ticks.saturating_sub(188).min(48) as u16 * 217 / 48;
        let ambient_strength = 34 + ticks.min(236) as u16 * 38 / 236;

        for (index, (source, destination)) in ORACLE_AWAKENING
            .as_chunks::<3>()
            .0
            .iter()
            .zip(self.pixels.as_chunks_mut::<4>().0.iter_mut())
            .enumerate()
        {
            let x = (index % WIDTH) as i32;
            let y = (index / WIDTH) as i32;
            let [red, green, blue] = [source[0] as u16, source[1] as u16, source[2] as u16];
            let in_oracle = (x - 120).abs() + (y - 80).abs() < 47;
            let cyan_pixel = blue > red.saturating_add(12) && green > red;
            let gold_pixel = red > blue.saturating_add(14) && green > blue;
            let strength = if in_oracle {
                center_strength
            } else if cyan_pixel {
                cyan_strength
            } else if gold_pixel {
                gold_strength
            } else {
                ambient_strength
            };
            destination.copy_from_slice(&[
                (red * strength / 255) as u8,
                (green * strength / 255) as u8,
                (blue * strength / 255) as u8,
                255,
            ]);
        }
    }

    fn blit_rgba(
        &mut self,
        rgba: &[u8],
        source_width: usize,
        source_height: usize,
        x: i32,
        y: i32,
        scale: i32,
    ) {
        debug_assert_eq!(rgba.len(), source_width * source_height * 4);
        for source_y in 0..source_height {
            for source_x in 0..source_width {
                let source_index = (source_y * source_width + source_x) * 4;
                let alpha = rgba[source_index + 3] as u16;
                if alpha == 0 {
                    continue;
                }
                for offset_y in 0..scale {
                    for offset_x in 0..scale {
                        let target_x = x + source_x as i32 * scale + offset_x;
                        let target_y = y + source_y as i32 * scale + offset_y;
                        if target_x < 0
                            || target_y < 0
                            || target_x >= WIDTH as i32
                            || target_y >= HEIGHT as i32
                        {
                            continue;
                        }
                        let destination_index = (target_y as usize * WIDTH + target_x as usize) * 4;
                        let inverse = 255 - alpha;
                        for channel in 0..3 {
                            let source_channel = rgba[source_index + channel] as u16;
                            let destination_channel =
                                self.pixels[destination_index + channel] as u16;
                            self.pixels[destination_index + channel] =
                                ((source_channel * alpha + destination_channel * inverse) / 255)
                                    as u8;
                        }
                        self.pixels[destination_index + 3] = 255;
                    }
                }
            }
        }
    }

    fn pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            return;
        }
        let index = (y as usize * WIDTH + x as usize) * 4;
        self.pixels[index..index + 4].copy_from_slice(&[color.0, color.1, color.2, 255]);
    }

    fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        for py in y.max(0)..(y + height).min(HEIGHT as i32) {
            for px in x.max(0)..(x + width).min(WIDTH as i32) {
                self.pixel(px, py, color);
            }
        }
    }

    fn outline(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        self.rect(x, y, width, 1, color);
        self.rect(x, y + height - 1, width, 1, color);
        self.rect(x, y, 1, height, color);
        self.rect(x + width - 1, y, 1, height, color);
    }

    fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let step_x = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let step_y = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.pixel(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let doubled = error * 2;
            if doubled >= dy {
                error += dy;
                x0 += step_x;
            }
            if doubled <= dx {
                error += dx;
                y0 += step_y;
            }
        }
    }

    fn text(&mut self, x: i32, y: i32, text: &str, color: Color, scale: i32) {
        let mut cursor = x;
        for ch in text.to_ascii_uppercase().chars() {
            for (gy, row) in glyph(ch).iter().enumerate() {
                for gx in 0..GLYPH_WIDTH {
                    let mask = 1u8 << (GLYPH_WIDTH - 1 - gx) as u32;
                    if row & mask != 0 {
                        self.rect(
                            cursor + gx * scale,
                            y + gy as i32 * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
            cursor += GLYPH_ADVANCE * scale;
        }
    }

    fn compact_text(&mut self, x: i32, y: i32, text: &str, color: Color) {
        let mut cursor = x;
        for ch in text.to_ascii_uppercase().chars() {
            for (glyph_y, row) in glyph(ch).iter().enumerate() {
                for glyph_x in 0..GLYPH_WIDTH {
                    let mask = 1u8 << (GLYPH_WIDTH - 1 - glyph_x) as u32;
                    if row & mask != 0 {
                        self.pixel(cursor + glyph_x, y + glyph_y as i32, color);
                    }
                }
            }
            cursor += GLYPH_WIDTH;
        }
    }

    fn centered_text(&mut self, y: i32, text: &str, color: Color, scale: i32) {
        let width = text_width(text, scale);
        self.text((WIDTH as i32 - width) / 2, y, text, color, scale);
    }

    fn centered_text_in(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        text: &str,
        color: Color,
        scale: i32,
    ) {
        let rendered_width = text_width(text, scale);
        debug_assert!(
            rendered_width <= width,
            "text `{text}` is wider than its {width}px container"
        );
        self.text(x + (width - rendered_width) / 2, y, text, color, scale);
    }

    fn centered_text_box(&mut self, bounds: UiBox, text: &str, color: Color, scale: i32) {
        let rendered_width = text_width(text, scale);
        let rendered_height = 7 * scale;
        debug_assert!(
            rendered_width <= bounds.width && rendered_height <= bounds.height,
            "text `{text}` does not fit {bounds:?}"
        );
        self.text(
            bounds.x + (bounds.width - rendered_width) / 2,
            bounds.y + (bounds.height - rendered_height) / 2,
            text,
            color,
            scale,
        );
    }

    fn centered_compact_text_box(&mut self, bounds: UiBox, text: &str, color: Color) {
        let rendered_width = text.chars().count() as i32 * GLYPH_WIDTH;
        debug_assert!(
            rendered_width <= bounds.width && 7 <= bounds.height,
            "compact text `{text}` does not fit {bounds:?}"
        );
        self.compact_text(
            bounds.x + (bounds.width - rendered_width) / 2,
            bounds.y + (bounds.height - 7) / 2,
            text,
            color,
        );
    }

    fn centered_compact_lines_box(&mut self, bounds: UiBox, lines: &[String], color: Color) {
        let capacity = ((bounds.height + 1) / LINE_HEIGHT) as usize;
        let visible = &lines[..lines.len().min(capacity)];
        let rendered_height = visible.len() as i32 * LINE_HEIGHT - 1;
        let start_y = bounds.y + (bounds.height - rendered_height) / 2;
        for (index, line) in visible.iter().enumerate() {
            self.centered_compact_text_box(
                UiBox {
                    y: start_y + index as i32 * LINE_HEIGHT,
                    height: 7,
                    ..bounds
                },
                line,
                color,
            );
        }
    }

    fn wrapped_text(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        max_chars: usize,
        max_lines: usize,
    ) {
        for (line_no, line) in wrap_text(text, max_chars)
            .into_iter()
            .take(max_lines)
            .enumerate()
        {
            self.text(x, y + line_no as i32 * LINE_HEIGHT, &line, color, 1);
        }
    }
}

#[derive(Clone, Debug)]
struct QuizRun {
    question: usize,
    completed_batches: usize,
    /// The focused display slot; `order` maps it to a source choice index.
    selected: usize,
    hearts: u8,
    score: u32,
    level: u32,
    streak: u32,
    leveled_up: bool,
    /// The committed result and its remaining input hold. The lesson card
    /// stays up after the hold reaches zero until A or Start continues.
    feedback: Option<(bool, u16)>,
    /// Display order of the current question: `order[slot]` is the source
    /// choice index drawn in that slot.
    order: Vec<usize>,
    /// The question index `order` and `attempt` were derived for.
    presented: Option<usize>,
    /// Earlier attempts at the current question's identity in this run.
    attempt: u32,
    /// Committed attempts per normalized question identity in this run.
    attempts: HashMap<String, u32>,
    /// True when the committed answer redeemed a previously missed question.
    redeemed: bool,
    /// Ticks left in which a second B leaves the run.
    leave_armed: u16,
}

impl QuizRun {
    fn new() -> Self {
        Self {
            question: 0,
            completed_batches: 0,
            selected: 0,
            hearts: 3,
            score: 0,
            level: 1,
            streak: 0,
            leveled_up: false,
            feedback: None,
            order: Vec::new(),
            presented: None,
            attempt: 0,
            attempts: HashMap::new(),
            redeemed: false,
            leave_armed: 0,
        }
    }

    /// The source choice indices in display order for a question with
    /// `choice_count` choices. Until the current question has been presented,
    /// choices keep their source order.
    fn display_order(&self, choice_count: usize) -> Vec<usize> {
        if self.presented == Some(self.question) && self.order.len() == choice_count {
            self.order.clone()
        } else {
            (0..choice_count).collect()
        }
    }

    /// The source choice index shown in display `slot`.
    fn source_choice(&self, slot: usize, choice_count: usize) -> usize {
        self.display_order(choice_count)
            .get(slot)
            .copied()
            .unwrap_or(slot)
    }
}

/// Normalized question identity: uppercase with collapsed whitespace, the same
/// rule the cartridge save uses to recognize answered questions.
fn question_identity(question: &str) -> String {
    question
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

/// Where a missed question's review copy is inserted: `RETRY_GAP` questions
/// after the current one, but never beyond the end of the current batch, so a
/// batch cannot complete before its misses are retried.
fn retry_insertion_index(current: usize, batch_end: Option<usize>, question_count: usize) -> usize {
    let batch_end = batch_end
        .unwrap_or(question_count)
        .clamp(current + 1, question_count.max(current + 1));
    (current + 1 + RETRY_GAP).min(batch_end)
}

/// Inserts a review copy of the missed question at `current` and shifts every
/// batch end at or beyond the insertion point so the copy joins the current
/// batch. Returns the insertion index.
fn schedule_retry(
    questions: &mut Vec<QuizQuestion>,
    batch_ends: &mut [usize],
    current: usize,
) -> Option<usize> {
    let mut review = questions.get(current)?.clone();
    review.review = true;
    let batch_end = batch_ends.iter().copied().find(|end| *end > current);
    let index = retry_insertion_index(current, batch_end, questions.len());
    questions.insert(index, review);
    for end in batch_ends.iter_mut().filter(|end| **end >= index) {
        *end += 1;
    }
    Some(index)
}

/// Upserts the journal entry for a committed question, keyed by identity. A
/// later correct attempt clears the entry's outstanding flag.
fn record_lesson(lessons: &mut Vec<Lesson>, question: &QuizQuestion, correct: bool) {
    let identity = question_identity(&question.question);
    let lesson = Lesson {
        question: question.question.clone(),
        answer: question
            .choices
            .get(question.answer)
            .cloned()
            .unwrap_or_default(),
        rationale: choice_rationale(question, question.answer)
            .unwrap_or_default()
            .to_string(),
        concept: question.concept,
        outstanding: !correct,
    };
    match lessons
        .iter_mut()
        .find(|entry| question_identity(&entry.question) == identity)
    {
        Some(entry) => *entry = lesson,
        None => lessons.push(lesson),
    }
}

/// The rationale for source choice `index`, when the question carries a full
/// set of rationales and that one is not blank.
fn choice_rationale(question: &QuizQuestion, index: usize) -> Option<&str> {
    if question.rationales.len() != question.choices.len() {
        return None;
    }
    question
        .rationales
        .get(index)
        .map(|rationale| rationale.trim())
        .filter(|rationale| !rationale.is_empty())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OracleDropKind {
    Data,
    Bug,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpeningBeat {
    Legacy,
    SourceEmber,
    ArchiveAnswer,
    MemoryVault,
    Convergence,
    OracleAwakening,
}

#[derive(Clone, Copy, Debug)]
struct OracleDrop {
    x: i32,
    y: i32,
    kind: OracleDropKind,
}

#[derive(Resource)]
struct GameState {
    powered: bool,
    ai_provider: Option<String>,
    cartridge: Option<CartridgeSpec>,
    machine: Option<SceneMachine>,
    screen: Screen,
    screen_ticks: u64,
    held: HashSet<Button>,
    menu_selected: usize,
    /// Codex page: 0 is the mastery overview, `n` is journal lesson `n - 1`.
    codex_page: usize,
    hero_row: usize,
    hero_name: usize,
    hero_class: usize,
    hero_style: usize,
    quest_selected: usize,
    quiz: Option<QuizRun>,
    batch_ends: Vec<usize>,
    pending_questions: Option<(String, Vec<QuizQuestion>)>,
    questions_loading: bool,
    question_retry_ticks: u16,
    oracle_hero_x: i32,
    oracle_drops: Vec<OracleDrop>,
    oracle_spawned: u32,
    oracle_data: u32,
    oracle_bug_hits: u32,
    logs: VecDeque<(String, bool)>,
    active_boss: String,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            powered: false,
            ai_provider: None,
            cartridge: None,
            machine: None,
            screen: Screen::Off,
            screen_ticks: 0,
            held: HashSet::new(),
            menu_selected: 0,
            codex_page: 0,
            hero_row: 0,
            hero_name: 0,
            hero_class: 0,
            hero_style: 0,
            quest_selected: 0,
            quiz: None,
            batch_ends: Vec::new(),
            pending_questions: None,
            questions_loading: false,
            question_retry_ticks: 0,
            oracle_hero_x: 104,
            oracle_drops: Vec::new(),
            oracle_spawned: 0,
            oracle_data: 0,
            oracle_bug_hits: 0,
            logs: VecDeque::new(),
            active_boss: String::new(),
        }
    }
}

impl GameState {
    fn ai_provider_name(&self) -> &str {
        self.ai_provider.as_deref().unwrap_or("AI")
    }

    fn ai_provider_status(&self, status: &str) -> String {
        format!("{}:{status}", self.ai_provider_name())
    }

    fn transition(&mut self, screen: Screen) {
        if screen == Screen::Oracle && self.screen != Screen::Oracle {
            self.oracle_hero_x = 104;
            self.oracle_drops.clear();
            self.oracle_spawned = 0;
        }
        if screen == Screen::Codex && self.screen != Screen::Codex {
            self.codex_page = 0;
        }
        self.screen = screen;
        self.screen_ticks = 0;
    }

    fn start_machine(&mut self) {
        let handler = self.machine.as_mut().map(|machine| {
            machine.reset();
            machine.current_handler()
        });
        if let Some(handler) = handler {
            self.transition(handler.into());
        }
    }

    fn signal(&mut self, signal: SceneSignal) -> bool {
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

    fn tick_machine(&mut self) -> bool {
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

    fn can_signal(&self, signal: SceneSignal) -> bool {
        self.machine
            .as_ref()
            .is_some_and(|machine| machine.can_handle(signal))
    }

    fn has_game(&self) -> bool {
        self.cartridge.is_some()
    }

    fn cartridge_mode(&self) -> Option<CartridgeMode> {
        self.cartridge.as_ref().map(CartridgeSpec::mode)
    }

    fn question_count(&self) -> usize {
        self.cartridge
            .as_ref()
            .map_or(0, |cart| cart.questions.len())
    }

    fn has_unanswered_question(&self) -> bool {
        let question = self.quiz.as_ref().map_or(0, |run| run.question);
        self.cartridge
            .as_ref()
            .is_some_and(|cartridge| cartridge.questions.get(question).is_some())
    }

    fn presentation_tier(&self) -> PresentationTier {
        PresentationTier::from_level(self.quiz.as_ref().map_or(1, |run| run.level))
    }

    fn visual_tier(&self) -> PresentationTier {
        if self.has_visual_template(VisualTemplate::Progression) {
            self.presentation_tier()
        } else {
            PresentationTier::Initiate
        }
    }

    fn has_visual_template(&self, template: VisualTemplate) -> bool {
        self.cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.codequest.as_deref())
            .is_some_and(|config| config.art.iter().any(|art| art.template == Some(template)))
    }

    fn uses_visual_template(&self, template: VisualTemplate) -> bool {
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

    fn uses_art(&self, expected_art_id: &str) -> bool {
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

    fn declares_art(&self, expected_art_id: &str) -> bool {
        self.cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.codequest.as_deref())
            .is_some_and(|config| config.art.iter().any(|art| art.id == expected_art_id))
    }

    fn opening_beat(&self) -> OpeningBeat {
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

    fn lessons(&self) -> &[Lesson] {
        self.cartridge
            .as_ref()
            .map_or(&[], |cartridge| cartridge.lessons.as_slice())
    }

    /// Lit mastery runes (0-3) for `concept` on the inserted cartridge.
    fn mastery_stage(&self, concept: Concept) -> usize {
        self.cartridge
            .as_ref()
            .and_then(|cartridge| cartridge.mastery.get(&concept))
            .map_or(0, LensRecord::stage)
    }

    fn codex_page_count(&self) -> usize {
        1 + self.lessons().len()
    }

    /// The journal lesson shown on the current Codex page, if any.
    fn codex_lesson(&self) -> Option<(usize, &Lesson)> {
        let index = self.codex_page.checked_sub(1)?;
        self.lessons().get(index).map(|lesson| (index, lesson))
    }

    /// A truthful one-line journal summary for the quiz menu, offered only when
    /// the menu can open the Codex that explains it.
    fn journal_summary(&self) -> Option<String> {
        let lessons = self.lessons();
        if lessons.is_empty() || !self.can_signal(SceneSignal::OpenCodex) {
            return None;
        }
        let pending = lessons.iter().filter(|lesson| lesson.outstanding).count();
        Some(if pending == 0 {
            format!("LESSONS {:02}  ALL CLEAR", lessons.len().min(99))
        } else {
            format!(
                "LESSONS {:02}  REVIEW {:02}",
                lessons.len().min(99),
                pending.min(99)
            )
        })
    }
}

fn request_question_batch(state: &mut GameState, effects: &mut Effects, level: u32) {
    if state.questions_loading
        || state.pending_questions.is_some()
        || state.question_retry_ticks > 0
    {
        return;
    }
    let Some(cartridge) = state
        .cartridge
        .as_ref()
        .filter(|cartridge| cartridge.mode() == CartridgeMode::Quiz)
    else {
        return;
    };
    effects.0.push_back(EngineEffect::RequestQuestions {
        cartridge_id: cartridge.id.clone(),
        level,
        count: QUESTION_BATCH_SIZE,
    });
    state.questions_loading = true;
}

fn begin_quiz_run(state: &mut GameState) {
    state.oracle_data = 0;
    state.oracle_bug_hits = 0;
    state.quiz = Some(QuizRun::new());
    state.signal(SceneSignal::HeroReady);
}

/// Derives the display order once for the question that just became current.
/// The attempt number counts this run's earlier commitments at the same
/// identity; a question that arrives flagged for review starts at attempt 1,
/// so it never reappears in the order the player first saw.
fn present_current_question(state: &mut GameState) {
    let Some(run) = state.quiz.as_mut() else {
        return;
    };
    if run.presented == Some(run.question) {
        return;
    }
    let Some(question) = state
        .cartridge
        .as_ref()
        .and_then(|cartridge| cartridge.questions.get(run.question))
    else {
        return;
    };
    let identity = question_identity(&question.question);
    let attempt = run
        .attempts
        .get(&identity)
        .copied()
        .unwrap_or(u32::from(question.review));
    run.order = if question.choices.len() == 4 {
        learning::presentation_order(&identity, attempt).to_vec()
    } else {
        (0..question.choices.len()).collect()
    };
    run.attempt = attempt;
    run.presented = Some(run.question);
    run.selected = 0;
}

/// Commits the focused choice: scores it, records evidence and the lesson,
/// schedules a spaced retry for a survivable miss, and opens the lesson card.
fn commit_answer(state: &mut GameState, effects: &mut Effects) {
    let (Some(run), Some(cartridge)) = (state.quiz.as_mut(), state.cartridge.as_mut()) else {
        return;
    };
    let Some(question) = cartridge.questions.get(run.question).cloned() else {
        return;
    };
    let picked = run.source_choice(run.selected, question.choices.len());
    let correct = picked == question.answer;
    if correct {
        run.streak += 1;
        run.score = run.score.saturating_add(score_award_for_streak(run.streak));
    } else {
        run.hearts = run.hearts.saturating_sub(1);
        run.streak = 0;
    }
    run.feedback = Some((correct, QUIZ_FEEDBACK_TICKS));
    run.redeemed = correct && question.review;
    run.attempts.insert(
        question_identity(&question.question),
        run.attempt.saturating_add(1),
    );

    let evidence = AnswerEvidence {
        question: question.question.clone(),
        concept: question.concept,
        correct,
        review: question.review,
    };
    learning::record_evidence(&mut cartridge.mastery, &evidence);
    record_lesson(&mut cartridge.lessons, &question, correct);
    if !correct && run.hearts > 0 {
        schedule_retry(
            &mut cartridge.questions,
            &mut state.batch_ends,
            run.question,
        );
    }
    effects.0.push_back(EngineEffect::RecordAnsweredQuestion {
        cartridge_id: cartridge.id.clone(),
        evidence,
    });
}

/// Leaves the lesson card: ends the run on a broken ward, otherwise advances
/// to the next question and completes the batch exactly at its (possibly
/// retry-shifted) end.
fn continue_after_lesson(state: &mut GameState) {
    let question_count = state.question_count();
    let batch_ends = &state.batch_ends;
    let Some(run) = state.quiz.as_mut() else {
        return;
    };
    let next_batch_end = batch_ends.get(run.completed_batches).copied();
    run.feedback = None;
    run.redeemed = false;
    let mut next_signal = None;
    if run.hearts == 0 {
        next_signal = Some(SceneSignal::HeartsEmpty);
    } else {
        run.question += 1;
        run.selected = 0;
        if next_batch_end == Some(run.question) {
            run.completed_batches += 1;
            run.level += 1;
            run.leveled_up = true;
        }
        if run.leveled_up {
            run.leveled_up = false;
            next_signal = Some(SceneSignal::BatchComplete);
        } else if run.question >= question_count {
            next_signal = Some(SceneSignal::NeedsQuestion);
        }
    }
    if let Some(signal) = next_signal {
        state.signal(signal);
    }
    if state.screen == Screen::Quiz {
        present_current_question(state);
    }
}

fn cycle_index(index: &mut usize, count: usize, direction: isize) {
    *index = (*index as isize + direction).rem_euclid(count as isize) as usize;
}

fn adjust_hero(state: &mut GameState, direction: isize) {
    match state.hero_row {
        0 => cycle_index(&mut state.hero_name, HERO_NAMES.len(), direction),
        1 => cycle_index(&mut state.hero_class, HERO_CLASSES.len(), direction),
        2 => cycle_index(&mut state.hero_style, HERO_STYLES.len(), direction),
        _ => {}
    }
}

pub struct GameEngine {
    app: App,
}

impl GameEngine {
    pub fn new() -> Self {
        let mut app = App::new();
        app.init_resource::<Inbox>()
            .init_resource::<Effects>()
            .init_resource::<Framebuffer>()
            .init_resource::<GameState>()
            .add_systems(Update, (apply_commands, advance_game, render).chain());
        let mut engine = Self { app };
        engine.update();
        engine
    }

    fn command(&mut self, command: EngineCommand) {
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

    fn take_effects(&mut self) -> Vec<EngineEffect> {
        self.app
            .world_mut()
            .resource_mut::<Effects>()
            .0
            .drain(..)
            .collect()
    }

    #[cfg(test)]
    fn screen(&self) -> Screen {
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
    sender: mpsc::Sender<EngineCommand>,
    frame: Arc<RwLock<Vec<u8>>>,
}

impl EngineRuntime {
    pub fn spawn(
        question_loader: QuestionLoader,
        answered_question_recorder: AnsweredQuestionRecorder,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let frame = Arc::new(RwLock::new(vec![0; FRAME_BYTES]));
        let shared_frame = Arc::clone(&frame);
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
                    next_frame += FRAME_TIME;
                    if let Some(remaining) = next_frame.checked_duration_since(Instant::now()) {
                        thread::sleep(remaining);
                    } else {
                        next_frame = Instant::now();
                    }
                }
            })
            .expect("failed to start Bevy engine thread");

        Self { sender, frame }
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

    fn send(&self, command: EngineCommand) -> Result<(), String> {
        self.sender
            .send(command)
            .map_err(|_| "BEVY ENGINE STOPPED".to_string())
    }
}

fn apply_commands(
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
                state.logs.clear();
                effects.0.push_back(EngineEffect::AbortQuest);
                state.transition(if powered { Screen::Boot } else { Screen::Off });
            }
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
                state.batch_ends = state
                    .cartridge
                    .as_ref()
                    .filter(|cartridge| cartridge.mode() == CartridgeMode::Quiz)
                    .map(|cartridge| cartridge.question_batch_ends.clone())
                    .unwrap_or_default();
                if state.batch_ends.is_empty() {
                    if let Some(batch_end) = state.cartridge.as_ref().and_then(|cartridge| {
                        (cartridge.mode() == CartridgeMode::Quiz && !cartridge.questions.is_empty())
                            .then_some(cartridge.questions.len())
                    }) {
                        state.batch_ends.push(batch_end);
                    }
                }
                state.pending_questions = None;
                state.questions_loading = false;
                state.question_retry_ticks = 0;
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
                questions,
            } => {
                let is_current_quiz = state.cartridge.as_ref().is_some_and(|cartridge| {
                    cartridge.id == cartridge_id && cartridge.mode() == CartridgeMode::Quiz
                });
                if !is_current_quiz {
                    continue;
                }
                state.questions_loading = false;
                if questions.is_empty() {
                    state.question_retry_ticks = 300;
                    continue;
                }
                state.question_retry_ticks = 0;
                if state.screen == Screen::Quiz {
                    state.pending_questions = Some((cartridge_id, questions));
                    continue;
                }
                let batch_end = state.cartridge.as_mut().map(|cartridge| {
                    cartridge.questions.extend(questions);
                    cartridge.questions.len()
                });
                if let Some(batch_end) = batch_end {
                    state.batch_ends.push(batch_end);
                }
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
            EngineCommand::QuestOutput { line, stderr } => {
                if state.screen == Screen::Battle {
                    for wrapped in wrap_text(&line, 37).into_iter().take(3) {
                        state.logs.push_back((wrapped, stderr));
                    }
                    while state.logs.len() > 7 {
                        state.logs.pop_front();
                    }
                }
            }
            EngineCommand::QuestDone { success } => {
                if state.screen == Screen::Battle {
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

fn handle_press(state: &mut GameState, effects: &mut Effects, button: Button) {
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
                state.signal(SceneSignal::Back);
            }
        }
        Screen::Quiz => {
            present_current_question(state);
            let Some(run) = state.quiz.as_mut() else {
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
                Button::B if leave_armed => {
                    state.signal(SceneSignal::Back);
                }
                Button::B => run.leave_armed = QUIZ_LEAVE_CONFIRM_TICKS,
                Button::A => commit_answer(state, effects),
                _ => {}
            }
        }
        Screen::LevelUp => {
            if state.screen_ticks >= LEVEL_UP_HOLD_TICKS
                && matches!(button, Button::A | Button::Start)
            {
                let signal = if state.has_unanswered_question() {
                    SceneSignal::QuestionsReady
                } else {
                    SceneSignal::NeedsQuestion
                };
                state.signal(signal);
            }
        }
        Screen::GameOver => {
            if matches!(button, Button::A | Button::B | Button::Start) {
                state.signal(SceneSignal::Replay);
            }
        }
        Screen::Codex => {
            // Paging wraps between the mastery overview and the newest lesson.
            // A and Start are deliberately inactive: the Codex is read-only.
            let pages = state.codex_page_count();
            match button {
                Button::Left | Button::Up | Button::L => {
                    cycle_index(&mut state.codex_page, pages, -1)
                }
                Button::Right | Button::Down | Button::R => {
                    cycle_index(&mut state.codex_page, pages, 1)
                }
                Button::B => {
                    state.signal(SceneSignal::Back);
                }
                _ => {}
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
                        effects.0.push_back(EngineEffect::RunQuest(quest.command));
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

fn advance_game(mut state: ResMut<GameState>, mut effects: ResMut<Effects>) {
    state.screen_ticks = state.screen_ticks.saturating_add(1);
    if !matches!(state.screen, Screen::Off | Screen::Boot) {
        state.tick_machine();
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
        if let Some((cartridge_id, questions)) = state.pending_questions.take() {
            let batch_end = if let Some(cartridge) = state.cartridge.as_mut() {
                if cartridge.id == cartridge_id && !questions.is_empty() {
                    cartridge.questions.extend(questions);
                    Some(cartridge.questions.len())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(batch_end) = batch_end {
                state.batch_ends.push(batch_end);
            }
        }
    }
    match state.screen {
        Screen::Oracle if !state.has_unanswered_question() && !state.questions_loading => {
            let level = state.quiz.as_ref().map_or(1, |run| run.level);
            request_question_batch(&mut state, &mut effects, level);
        }
        Screen::Oracle if state.screen_ticks >= 75 && state.has_unanswered_question() => {
            state.signal(SceneSignal::QuestionsReady);
        }
        Screen::Quiz if !state.has_unanswered_question() => {
            state.signal(SceneSignal::NeedsQuestion);
        }
        Screen::Quiz => {
            let prefetch_level = state.quiz.as_ref().and_then(|run| {
                let remaining = state.question_count().saturating_sub(run.question);
                (remaining <= QUESTION_BATCH_SIZE
                    && !state.questions_loading
                    && state.pending_questions.is_none())
                .then_some(run.level + 1)
            });
            if let Some(level) = prefetch_level {
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
            let signal = if state.has_unanswered_question() {
                SceneSignal::QuestionsReady
            } else {
                SceneSignal::NeedsQuestion
            };
            state.signal(signal);
        }
        _ => {}
    }
    if state.screen == Screen::Quiz {
        present_current_question(&mut state);
    }
    state.question_retry_ticks = state.question_retry_ticks.saturating_sub(1);
}

fn render(mut frame: ResMut<Framebuffer>, state: Res<GameState>) {
    match state.screen {
        Screen::Off => frame.clear(INK),
        Screen::Boot => render_boot(&mut frame, &state),
        Screen::Copyright => render_copyright(&mut frame, &state),
        Screen::OpeningFanfare => render_opening_fanfare(&mut frame, &state),
        Screen::Title => render_title(&mut frame, &state),
        Screen::QuizMenu => render_quiz_menu(&mut frame, &state),
        Screen::CharacterCreation => render_character_creation(&mut frame, &state),
        Screen::Oracle => render_oracle(&mut frame, &state),
        Screen::Quiz => render_quiz(&mut frame, &state),
        Screen::LevelUp => render_level_up(&mut frame, &state),
        Screen::GameOver => render_game_over(&mut frame, &state),
        Screen::Codex => render_codex(&mut frame, &state),
        Screen::QuestSelect => render_quest_select(&mut frame, &state),
        Screen::Battle => render_battle(&mut frame, &state),
        Screen::Victory => render_result(&mut frame, true),
        Screen::Defeat => render_result(&mut frame, false),
    }
}

fn render_boot(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb_graded(ORACLE_GATEWAY, 92, 105, 74);
    frame.centered_text_box(GATEWAY_TITLE_TOP_BOX, "CODE QUEST", PARCH, 2);
    frame.centered_text_box(GATEWAY_TITLE_BOTTOM_BOX, "ADVANCE", AMBER, 1);
    frame.centered_text_box(GATEWAY_SIGNATURE_BOX, "REPOSITORY ORACLE", CYAN_DIM, 1);
    if !state.has_game() && state.screen_ticks > 50 && (state.screen_ticks / 30).is_multiple_of(2) {
        frame.centered_text_box(GATEWAY_PROMPT_BOX, "INSERT CARTRIDGE", PARCH, 1);
    }
}

fn draw_asset_focus(frame: &mut Framebuffer, x: i32, y: i32, width: i32, height: i32) {
    frame.outline(x, y, width, height, AMBER);
    for (corner_x, direction_x) in [(x, 1), (x + width - 1, -1)] {
        for corner_y in [y, y + height - 1] {
            let start_x = if direction_x > 0 {
                corner_x
            } else {
                corner_x - 3
            };
            frame.rect(start_x, corner_y, 4, 2, PARCH);
        }
    }
}

fn render_oracle_chronicle(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_CHRONICLE);
    frame.centered_compact_text_box(CHRONICLE_HEADER_BOX, "REPOSITORY CHRONICLE", MIST);
    if state.screen_ticks >= 42 {
        frame.rect(42, 122, 156, 17, VOID);
        frame.outline(42, 122, 156, 17, CYAN_DIM);
    }

    let title = state
        .cartridge
        .as_ref()
        .map_or("NO CARTRIDGE", |cartridge| cartridge.title.as_str());
    let title_lines = wrap_text(title, 23)
        .into_iter()
        .map(|line| truncate(&line, 23))
        .collect::<Vec<_>>();
    frame.centered_compact_lines_box(CHRONICLE_TITLE_BOX, &title_lines, PARCH);

    if let Some(provenance) = state
        .cartridge
        .as_ref()
        .map(|cartridge| &cartridge.provenance)
    {
        if state.screen_ticks >= 12 {
            let mut notice = provenance
                .copyright
                .as_deref()
                .map(|notice| notice.replace('©', "(C)"))
                .unwrap_or_else(|| "NO DECLARED COPYRIGHT NOTICE".into());
            if notice
                .get(..10)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("copyright "))
            {
                notice.drain(..10);
            }
            notice = notice.replace("(C)", "COPY").replace("(c)", "COPY");
            let notice_lines = wrap_text(&notice, 23)
                .into_iter()
                .map(|line| truncate(&line, 23))
                .collect::<Vec<_>>();
            frame.centered_compact_lines_box(CHRONICLE_COPYRIGHT_BOX, &notice_lines, MIST);
        }
        if state.screen_ticks >= 24 {
            frame.centered_compact_text_box(
                CHRONICLE_AUTHORS_LABEL_BOX,
                "COMMIT AUTHORS",
                CYAN_DIM,
            );
            if provenance.authors.is_empty() {
                frame.centered_compact_text_box(CHRONICLE_AUTHORS_BOX, "NO AUTHORS YET", PARCH);
            } else {
                let authors = provenance
                    .authors
                    .iter()
                    .take(3)
                    .map(|author| truncate(author, 23))
                    .collect::<Vec<_>>();
                frame.centered_compact_lines_box(CHRONICLE_AUTHORS_BOX, &authors, PARCH);
            }
        }
        if state.screen_ticks >= 42 {
            let history = match (provenance.first_year, provenance.latest_year) {
                (Some(first), Some(latest)) if first == latest => format!("ARCHIVE YEAR {first}"),
                (Some(first), Some(latest)) => format!("ARCHIVE {first} > {latest}"),
                _ => "HISTORY NOT YET WRITTEN".into(),
            };
            frame.centered_text_in(42, 126, 156, &truncate(&history, 24), MIST, 1);
        }
    }
    if state.can_signal(SceneSignal::Continue) && (state.screen_ticks / 20).is_multiple_of(2) {
        frame.rect(74, 141, 92, 16, VOID);
        frame.outline(74, 141, 92, 16, CYAN_DIM);
        frame.centered_text_in(74, 145, 92, "A / START:SKIP", MIST, 1);
    }
}

fn render_oracle_opening_beat(frame: &mut Framebuffer, beat: OpeningBeat, ticks: u64) {
    match beat {
        OpeningBeat::Legacy => frame.blit_awakening(ticks),
        OpeningBeat::SourceEmber => frame.blit_rgb(ORACLE_AWAKENING_SOURCE),
        OpeningBeat::ArchiveAnswer => frame.blit_rgb(ORACLE_AWAKENING_SIGNAL),
        OpeningBeat::MemoryVault => frame.blit_rgb(ORACLE_AWAKENING_ARCHIVE),
        OpeningBeat::Convergence => frame.blit_rgb(ORACLE_AWAKENING_CONVERGENCE),
        OpeningBeat::OracleAwakening => frame.blit_awakening(ticks.saturating_add(188)),
    }
}

fn render_oracle_awakening(frame: &mut Framebuffer, state: &GameState) {
    render_oracle_opening_beat(frame, state.opening_beat(), state.screen_ticks);
    if state.can_signal(SceneSignal::Continue) {
        frame.rect(164, 149, 76, 11, VOID);
        frame.text(166, 151, "A/START:SKIP", MIST, 1);
    }
}

fn render_oracle_title(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_GATEWAY);
    let title = state
        .cartridge
        .as_ref()
        .map_or("NO CARTRIDGE", |cart| cart.title.as_str());
    let lines = wrap_text(title, 10);
    frame.centered_text_box(GATEWAY_TITLE_TOP_BOX, &truncate(&lines[0], 10), PARCH, 2);
    if let Some(line) = lines.get(1) {
        frame.centered_text_box(GATEWAY_TITLE_BOTTOM_BOX, &truncate(line, 19), AMBER, 1);
    }
    frame.centered_text_box(GATEWAY_SIGNATURE_BOX, "REPOSITORY ORACLE", CYAN, 1);
    if state.has_game() && (state.screen_ticks / 30).is_multiple_of(2) {
        frame.centered_text_box(GATEWAY_PROMPT_BOX, "PRESS START", PARCH, 1);
    }
}

fn render_copyright(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Chronicle) {
        render_oracle_chronicle(frame, state);
        return;
    }
    frame.clear(INK);
    frame.outline(8, 8, 224, 144, GOLD);
    frame.outline(12, 12, 216, 136, NAVY);
    frame.centered_text(20, "REPOSITORY CHRONICLE", GOLD, 1);

    let title = state
        .cartridge
        .as_ref()
        .map_or("NO CARTRIDGE", |cartridge| cartridge.title.as_str());
    let lines = title_lines(title);
    frame.centered_text(40, &lines[0], PARCH, 1);
    if let Some(line) = lines.get(1) {
        frame.centered_text(51, line, PARCH, 1);
    }

    if let Some(provenance) = state
        .cartridge
        .as_ref()
        .map(|cartridge| &cartridge.provenance)
    {
        let notice = provenance
            .copyright
            .as_deref()
            .map(|notice| notice.replace('©', "(C)"))
            .unwrap_or_else(|| "NO COPYRIGHT NOTICE FOUND".into());
        frame.centered_text(66, &truncate(&notice, 35), MIST, 1);
        frame.rect(42, 79, 156, 1, PLUM);
        frame.centered_text(86, "AUTHORS", SKY, 1);

        if provenance.authors.is_empty() {
            frame.centered_text(99, "NO COMMIT AUTHORS YET", PARCH, 1);
        } else {
            for (index, author) in provenance.authors.iter().take(3).enumerate() {
                frame.centered_text(98 + index as i32 * 10, &truncate(author, 32), PARCH, 1);
            }
        }

        let history = match (provenance.first_year, provenance.latest_year) {
            (Some(first), Some(latest)) if first == latest => format!("HISTORY {first}"),
            (Some(first), Some(latest)) => format!("HISTORY {first}-{latest}"),
            _ => "HISTORY NOT YET WRITTEN".into(),
        };
        frame.centered_text(129, &history, GOLD, 1);
    }
    if state.can_signal(SceneSignal::Continue) && (state.screen_ticks / 20).is_multiple_of(2) {
        frame.centered_text(141, "START:SKIP", PARCH, 1);
    }
}

fn draw_code_sigil(frame: &mut Framebuffer, x: i32, y: i32, mirrored: bool) {
    let edge = if mirrored { -1 } else { 1 };
    frame.line(x, y - 16, x + edge * 12, y, SKY);
    frame.line(x + edge * 12, y, x, y + 16, SKY);
    frame.line(x + edge * 5, y - 16, x + edge * 17, y, ROYAL);
    frame.line(x + edge * 17, y, x + edge * 5, y + 16, ROYAL);
    frame.rect(x + edge.min(0) * 18, y - 2, 18, 4, GOLD);
}

fn draw_oracle_sigil(frame: &mut Framebuffer, center_x: i32, center_y: i32, pulse: i32) {
    let radius = 24 + pulse;
    frame.line(center_x - radius, center_y, center_x, center_y - 13, SKY);
    frame.line(center_x, center_y - 13, center_x + radius, center_y, SKY);
    frame.line(center_x + radius, center_y, center_x, center_y + 13, ROYAL);
    frame.line(center_x, center_y + 13, center_x - radius, center_y, ROYAL);
    frame.outline(center_x - 7, center_y - 7, 15, 15, GOLD);
    frame.rect(center_x - 2, center_y - 2, 5, 5, PARCH);
}

fn draw_commit_constellation(frame: &mut Framebuffer, ticks: u64) {
    let nodes = [
        (36, 98),
        (70, 72),
        (106, 91),
        (142, 62),
        (178, 80),
        (208, 48),
    ];
    for pair in nodes.windows(2) {
        frame.line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, PLUM);
    }
    for (index, (x, y)) in nodes.into_iter().enumerate() {
        let color = if ((ticks / 10) as usize + index).is_multiple_of(3) {
            GOLD
        } else {
            SKY
        };
        frame.rect(x - 2, y - 2, 5, 5, color);
    }
}

fn render_opening_fanfare(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Awakening) {
        render_oracle_awakening(frame, state);
        return;
    }
    let ticks = state.screen_ticks;
    frame.clear(INK);
    for index in 0..24 {
        let x = ((index * 67 + ticks as usize) % WIDTH) as i32;
        let y = ((index * 43 + 17) % HEIGHT) as i32;
        frame.pixel(x, y, if index % 4 == 0 { GOLD } else { MIST });
    }

    if ticks < 120 {
        let travel = (ticks.min(110) as i32 * 70) / 110;
        draw_code_sigil(frame, 24 + travel, 78, false);
        draw_code_sigil(frame, 216 - travel, 78, true);
        if ticks >= 100 {
            let flare = ((ticks - 100) as i32 / 4).min(8);
            frame.rect(120 - flare, 78 - 1, flare * 2 + 1, 3, PARCH);
            frame.rect(119, 79 - flare, 3, flare * 2 + 1, GOLD);
        }
        frame.centered_text(132, "TWO PATHS CONVERGE", SKY, 1);
    } else {
        draw_commit_constellation(frame, ticks);
        draw_oracle_sigil(frame, 120, 78, ((ticks / 10) % 3) as i32);
        frame.centered_text(20, "HISTORY BECOMES POWER", GOLD, 1);
        frame.centered_text(135, "THE ORACLE OPENS", PARCH, 1);
    }

    if state.can_signal(SceneSignal::Continue) {
        frame.text(176, 149, "START:SKIP", MIST, 1);
    }
}

fn render_title(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Title) {
        render_oracle_title(frame, state);
        return;
    }
    frame.clear(NAVY);
    for index in 0..42 {
        let x = ((index * 53 + state.screen_ticks as usize / 3) % WIDTH) as i32;
        let y = ((index * 37 + 11) % HEIGHT) as i32;
        frame.pixel(x, y, if index % 3 == 0 { SKY } else { MIST });
    }
    let title = state
        .cartridge
        .as_ref()
        .map_or("NO CARTRIDGE", |cart| cart.title.as_str());
    let lines = title_lines(title);
    frame.centered_text(42, &lines[0], PARCH, 2);
    if let Some(line) = lines.get(1) {
        frame.centered_text(61, line, GOLD, 2);
    }
    let subtitle = match state.cartridge_mode() {
        Some(CartridgeMode::Quiz) => "ENDLESS REPO QUIZ",
        Some(CartridgeMode::Custom) => "EVERY COMMAND IS A BOSS",
        None => "POWER OFF TO LOAD A GAME",
    };
    frame.centered_text(91, subtitle, SKY, 1);
    if state.has_game() && (state.screen_ticks / 30).is_multiple_of(2) {
        frame.centered_text(126, "PRESS START", PARCH, 1);
    }
}

fn quiz_menu_second_option(state: &GameState) -> &'static str {
    if state.can_signal(SceneSignal::OpenCodex) {
        "OPEN THE CODEX"
    } else {
        "RETURN TO TITLE"
    }
}

fn render_quiz_menu(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Menu) {
        render_oracle_menu(frame, state);
        return;
    }
    frame.clear(NAVY);
    frame.centered_text(20, "REPO QUIZ", GOLD, 2);
    if let Some(summary) = state.journal_summary() {
        frame.centered_text(44, &summary, SKY, 1);
    }
    frame.outline(34, 58, 172, 62, SKY);
    for (index, label) in ["BEGIN RUN", quiz_menu_second_option(state)]
        .iter()
        .enumerate()
    {
        let y = 74 + index as i32 * 24;
        if state.menu_selected == index {
            frame.rect(43, y - 3, 154, 14, ROYAL);
            frame.text(47, y, ">", GOLD, 1);
        }
        frame.text(59, y, label, PARCH, 1);
    }
    frame.centered_text(140, "A:CHOOSE  B:BACK", MIST, 1);
}

fn render_oracle_menu(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_GATEWAY);
    frame.centered_text_box(GATEWAY_MENU_HEADING_BOX, "CHOOSE YOUR PATH", PARCH, 1);
    let subtitle = state
        .journal_summary()
        .unwrap_or_else(|| "THE BOND BEGINS HERE".into());
    frame.centered_text_box(GATEWAY_MENU_SUBTITLE_BOX, &subtitle, CYAN, 1);
    for (index, label) in ["BEGIN THE TRIAL", quiz_menu_second_option(state)]
        .iter()
        .enumerate()
    {
        let option_box = GATEWAY_MENU_OPTION_BOXES[index];
        let text_box = GATEWAY_MENU_OPTION_TEXT_BOXES[index];
        let focused = state.menu_selected == index;
        if focused {
            draw_asset_focus(
                frame,
                option_box.x,
                option_box.y,
                option_box.width,
                option_box.height,
            );
        }
        frame.centered_text_box(text_box, label, if focused { PARCH } else { MIST }, 1);
    }
    frame.rect(
        MENU_FOOTER_BOX.x,
        MENU_FOOTER_BOX.y,
        MENU_FOOTER_BOX.width,
        MENU_FOOTER_BOX.height,
        VOID,
    );
    frame.outline(
        MENU_FOOTER_BOX.x,
        MENU_FOOTER_BOX.y,
        MENU_FOOTER_BOX.width,
        MENU_FOOTER_BOX.height,
        CYAN_DIM,
    );
    frame.centered_text_box(MENU_FOOTER_BOX, "D-PAD  A:CHOOSE  B:BACK", MIST, 1);
}

fn render_character_creation(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Atelier) {
        render_oracle_atelier(frame, state);
        return;
    }
    frame.clear(NAVY);
    frame.centered_text(9, "CREATE YOUR HERO", GOLD, 1);
    let bob = ((state.screen_ticks / 20) % 2) as i32;
    draw_hero(frame, 26, 64 - bob, 1, state);

    let rows = [
        format!("NAME   < {} >", HERO_NAMES[state.hero_name]),
        format!("CLASS  < {} >", HERO_CLASSES[state.hero_class]),
        format!("STYLE  < {} >", HERO_STYLES[state.hero_style]),
        "BEGIN QUEST".to_string(),
    ];
    for (index, label) in rows.iter().enumerate() {
        let y = 30 + index as i32 * 21;
        if state.hero_row == index {
            frame.rect(65, y - 3, 169, 14, ROYAL);
            frame.text(69, y, ">", GOLD, 1);
        }
        frame.text(81, y, label, PARCH, 1);
    }
    let oracle_status = if state.question_count() > 0 {
        "ORACLE READY"
    } else if state.questions_loading {
        "ORACLE IS WRITING..."
    } else {
        "ORACLE WILL RETRY..."
    };
    frame.centered_text(120, oracle_status, SKY, 1);
    frame.centered_text(138, "D-PAD:EDIT  A:CHOOSE", PARCH, 1);
    frame.centered_text(150, "START:BEGIN  B:BACK", MIST, 1);
}

fn render_oracle_atelier(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_ATELIER);
    frame.rect(
        ATELIER_HEADER_BOX.x,
        ATELIER_HEADER_BOX.y,
        ATELIER_HEADER_BOX.width,
        ATELIER_HEADER_BOX.height,
        VOID,
    );
    frame.outline(
        ATELIER_HEADER_BOX.x,
        ATELIER_HEADER_BOX.y,
        ATELIER_HEADER_BOX.width,
        ATELIER_HEADER_BOX.height,
        CYAN_DIM,
    );
    frame.centered_compact_text_box(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER", AMBER);
    let bob = ((state.screen_ticks / 22) % 2) as i32;
    draw_hero(
        frame,
        ATELIER_HERO_X,
        ATELIER_HERO_Y - bob,
        ATELIER_HERO_SCALE,
        state,
    );

    let rows = [
        ("NAME", HERO_NAMES[state.hero_name]),
        ("PATH", HERO_CLASSES[state.hero_class]),
        ("AURA", HERO_STYLES[state.hero_style]),
    ];
    for (index, (label, value)) in rows.iter().enumerate() {
        let row_box = ATELIER_ROW_BOXES[index];
        let label_box = atelier_label_box(row_box);
        let value_box = atelier_value_box(row_box);
        let focused = state.hero_row == index;
        if focused {
            draw_asset_focus(frame, row_box.x, row_box.y, row_box.width, row_box.height);
        }
        frame.centered_text_box(label_box, label, if focused { AMBER } else { CYAN_DIM }, 1);
        frame.centered_compact_text_box(
            value_box,
            &format!("<{}>", truncate(value, 14)),
            if focused { PARCH } else { MIST },
        );
    }

    if state.hero_row == 3 {
        draw_asset_focus(
            frame,
            ATELIER_BIND_BOX.x,
            ATELIER_BIND_BOX.y,
            ATELIER_BIND_BOX.width,
            ATELIER_BIND_BOX.height,
        );
    }
    frame.centered_text_box(
        ATELIER_BIND_TEXT_BOX,
        "BIND",
        if state.hero_row == 3 { PARCH } else { MIST },
        1,
    );

    let oracle_status = if state.question_count() > 0 {
        "ORACLE READY"
    } else if state.questions_loading {
        "ORACLE IS WRITING"
    } else {
        "VISION CLOUDY - RETRYING"
    };
    frame.rect(0, 143, WIDTH as i32, 17, VOID);
    frame.text(5, 148, oracle_status, CYAN, 1);
    frame.text(174, 148, "START:BIND", MIST, 1);
}

fn render_oracle(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Sanctum) {
        render_oracle_sanctum(frame, state);
        return;
    }
    frame.clear(INK);
    for index in 0..30 {
        let x = ((index * 71 + state.screen_ticks as usize * 2) % WIDTH) as i32;
        frame.pixel(x, 14 + (index * 29 % 96) as i32, MIST);
    }
    frame.rect(0, 0, WIDTH as i32, 12, NAVY);
    frame.rect(0, 11, WIDTH as i32, 1, PLUM);
    frame.text(4, 2, "ORACLE DATAFALL", GOLD, 1);
    let status = if state.has_unanswered_question() {
        "QUESTION READY".to_string()
    } else if state.questions_loading {
        format!("{} THINKING", state.ai_provider_name())
    } else if state.question_retry_ticks > 0 {
        format!("{} RETRYING", state.ai_provider_name())
    } else {
        format!("CONTACTING {}", state.ai_provider_name())
    };
    let status_width = status.chars().count() as i32 * GLYPH_ADVANCE - 1;
    frame.text(211 - status_width, 2, &status, SKY, 1);
    let phase = (state.screen_ticks % 45) / 15;
    frame.text(216, 2, &".".repeat(phase as usize + 1), GOLD, 1);
    for drop in &state.oracle_drops {
        match drop.kind {
            OracleDropKind::Data => draw_oracle_data(frame, drop.x, drop.y),
            OracleDropKind::Bug => draw_oracle_bug(frame, drop.x, drop.y),
        }
    }
    frame.rect(0, 132, WIDTH as i32, 28, PLUM);
    frame.rect(0, 128, WIDTH as i32, 4, GREEN);
    draw_hero(frame, state.oracle_hero_x, 111, 1, state);
    frame.text(
        4,
        143,
        &format!("DATA {:02}", state.oracle_data.min(99)),
        GREEN,
        1,
    );
    let data_stage = threshold_stage(state.oracle_data, &DATA_CHARGE_THRESHOLDS);
    draw_oracle_rune_meter(
        frame,
        49,
        143,
        data_stage,
        if data_stage >= 2 { AMBER } else { CYAN },
    );
    frame.centered_text(143, "L/R MOVE  B:BACK", PARCH, 1);
    let breach_stage = threshold_stage(state.oracle_bug_hits, &BUG_BREACH_THRESHOLDS);
    let containment_color = match breach_stage {
        0 => CYAN,
        1 => AMBER,
        2 => MAGENTA,
        _ => RED,
    };
    draw_oracle_rune_meter(
        frame,
        173,
        143,
        3usize.saturating_sub(breach_stage),
        containment_color,
    );
    frame.text(
        201,
        143,
        &format!("BUG {:02}", state.oracle_bug_hits.min(99)),
        RED,
        1,
    );
}

fn render_oracle_sanctum(frame: &mut Framebuffer, state: &GameState) {
    let tier = state.visual_tier();
    match tier {
        PresentationTier::Initiate => frame.blit_rgb_graded(ORACLE_SANCTUM, 218, 238, 172),
        PresentationTier::Adept => frame.blit_rgb_graded(ORACLE_SANCTUM, 236, 250, 226),
        PresentationTier::OracleBound => frame.blit_rgb(ORACLE_SANCTUM),
    };
    frame.rect(0, 0, WIDTH as i32, 15, VOID);
    frame.rect(0, 143, WIDTH as i32, 17, VOID);
    frame.text(5, 5, "DATAFALL", AMBER, 1);
    frame.text(
        60,
        5,
        tier.label(),
        match tier {
            PresentationTier::Initiate => CYAN_DIM,
            PresentationTier::Adept => AMBER,
            PresentationTier::OracleBound => MAGENTA,
        },
        1,
    );
    let status = if state.has_unanswered_question() {
        state.ai_provider_status("READY")
    } else if state.questions_loading {
        state.ai_provider_status("SCRYING")
    } else if state.question_retry_ticks > 0 {
        state.ai_provider_status("CLOUDY")
    } else {
        state.ai_provider_status("CHANNEL")
    };
    let status_width = status.chars().count() as i32 * GLYPH_ADVANCE - 1;
    frame.text(235 - status_width, 5, &status, CYAN, 1);

    if tier == PresentationTier::OracleBound {
        for (x, y) in [
            (120, 28),
            (104, 38),
            (136, 38),
            (96, 52),
            (144, 52),
            (104, 66),
            (136, 66),
            (120, 76),
        ] {
            frame.rect(x - 2, y - 2, 5, 5, VOID);
            frame.outline(x - 2, y - 2, 5, 5, VIOLET);
            frame.pixel(x, y, MAGENTA);
        }
    }

    for drop in &state.oracle_drops {
        match drop.kind {
            OracleDropKind::Data => draw_oracle_data(frame, drop.x, drop.y),
            OracleDropKind::Bug => draw_oracle_bug(frame, drop.x, drop.y),
        }
    }

    draw_hero(frame, state.oracle_hero_x, 106, 1, state);
    frame.text(
        5,
        149,
        &format!("DATA {:02}", state.oracle_data.min(99)),
        GREEN,
        1,
    );
    let data_stage = threshold_stage(state.oracle_data, &DATA_CHARGE_THRESHOLDS);
    draw_oracle_rune_meter(
        frame,
        49,
        149,
        data_stage,
        if data_stage >= 2 { AMBER } else { CYAN },
    );
    frame.centered_text(149, "L/R:MOVE  B:LEAVE", PARCH, 1);
    let breach_stage = threshold_stage(state.oracle_bug_hits, &BUG_BREACH_THRESHOLDS);
    let containment_color = match breach_stage {
        0 => CYAN,
        1 => AMBER,
        2 => MAGENTA,
        _ => RED,
    };
    draw_oracle_rune_meter(
        frame,
        173,
        149,
        3usize.saturating_sub(breach_stage),
        containment_color,
    );
    frame.text(
        198,
        149,
        &format!("BUG {:02}", state.oracle_bug_hits.min(99)),
        RED,
        1,
    );
}

fn draw_oracle_data(frame: &mut Framebuffer, x: i32, y: i32) {
    let offset = DROP_SPRITE_SIZE as i32 / 2;
    frame.blit_rgba(
        ORACLE_DATA_DROP,
        DROP_SPRITE_SIZE,
        DROP_SPRITE_SIZE,
        x - offset,
        y - offset,
        1,
    );
}

fn draw_oracle_bug(frame: &mut Framebuffer, x: i32, y: i32) {
    let offset = DROP_SPRITE_SIZE as i32 / 2;
    frame.blit_rgba(
        ORACLE_BUG_DROP,
        DROP_SPRITE_SIZE,
        DROP_SPRITE_SIZE,
        x - offset,
        y - offset,
        1,
    );
}

fn draw_oracle_rune(frame: &mut Framebuffer, x: i32, y: i32, lit: bool, color: Color) {
    let outline = if lit { color } else { ASH };
    for (dx, dy) in [
        (2, 0),
        (1, 1),
        (3, 1),
        (0, 2),
        (4, 2),
        (0, 3),
        (4, 3),
        (0, 4),
        (4, 4),
        (1, 5),
        (3, 5),
        (2, 6),
    ] {
        frame.pixel(x + dx, y + dy, outline);
    }
    if lit {
        for (dx, dy) in [(2, 2), (1, 3), (2, 3), (3, 3), (2, 4)] {
            frame.pixel(x + dx, y + dy, PARCH);
        }
    }
}

fn draw_oracle_rune_meter(frame: &mut Framebuffer, x: i32, y: i32, lit_runes: usize, color: Color) {
    for index in 0..3 {
        draw_oracle_rune(frame, x + index as i32 * 7, y, index < lit_runes, color);
    }
}

fn ward_color(hearts: u8) -> Color {
    match hearts {
        3.. => CYAN,
        2 => AMBER,
        _ => RED,
    }
}

fn draw_oracle_ward_meter(frame: &mut Framebuffer, x: i32, y: i32, hearts: u8) {
    let color = ward_color(hearts);
    frame.text(x, y, "WARD", color, 1);
    draw_oracle_rune_meter(frame, x + 27, y, hearts.min(3) as usize, color);
}

fn crossed_insight_stage(run: &QuizRun) -> Option<InsightStage> {
    if !matches!(run.feedback, Some((true, _))) {
        return None;
    }
    let current = InsightStage::from_score(run.score);
    let previous_score = run.score.saturating_sub(score_award_for_streak(run.streak));
    let previous = InsightStage::from_score(previous_score);
    (current > previous).then_some(current)
}

fn quiz_feedback_banner(run: &QuizRun) -> String {
    match run.feedback {
        Some((true, _)) => {
            if let Some(stage) = crossed_insight_stage(run) {
                format!("RUNE {} AWAKENS", stage.label())
            } else if run.redeemed {
                "REDEEMED".into()
            } else if streak_multiplier(run.streak) > 1 {
                format!("FLOW X{}", streak_multiplier(run.streak))
            } else {
                "REVIEW ANSWER".into()
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

fn quiz_feedback_color(run: &QuizRun) -> Color {
    match run.feedback {
        Some((true, _)) if crossed_insight_stage(run).is_some() => AMBER,
        Some((true, _)) if run.redeemed => GREEN,
        Some((true, _)) => CYAN,
        Some((false, _)) => ward_color(run.hearts),
        None if run.leave_armed > 0 => AMBER,
        None => MIST,
    }
}

/// True once the lesson card's input hold has elapsed and A or Start will
/// continue.
fn lesson_is_live(run: &QuizRun) -> bool {
    matches!(run.feedback, Some((_, 0)))
}

const LESSON_CONTINUE: &str = "A:CONTINUE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LessonTone {
    /// The player's wrong pick: the misconception it reveals.
    Misconception,
    MisconceptionWhy,
    /// The correct answer and why it holds.
    Answer,
    AnswerWhy,
}

impl LessonTone {
    fn color(self) -> Color {
        match self {
            Self::Misconception => RED,
            Self::MisconceptionWhy => MIST,
            Self::Answer => GREEN,
            Self::AnswerWhy => PARCH,
        }
    }
}

/// One positioned line of lesson copy.
#[derive(Clone, Debug, Eq, PartialEq)]
struct LessonLine {
    x: i32,
    y: i32,
    text: String,
    tone: LessonTone,
}

/// Composes the lesson card copy inside `area`: for a miss, the player's pick
/// with its rationale and then the correct answer with its rationale; for a
/// success, the answer with its rationale. Choice lines carry a `-` or `+`
/// marker as well as color, so the verdict never depends on hue alone. The
/// fixed rationale column is centered horizontally and the whole composition
/// vertically; legacy questions without rationales show only choice lines.
fn lesson_lines(
    question: &QuizQuestion,
    picked: usize,
    correct: bool,
    area: UiBox,
) -> Vec<LessonLine> {
    let mut blocks: Vec<Vec<(String, LessonTone)>> = Vec::new();
    let mut block = |marker: &str, index: usize, tone: LessonTone, why: LessonTone| {
        let choice = question.choices.get(index).map_or("", String::as_str);
        let mut lines = vec![(
            format!("{marker} {}", truncate(choice, QUIZ_CHOICE_CHARS)),
            tone,
        )];
        if let Some(rationale) = choice_rationale(question, index) {
            lines.extend(
                wrap_text(rationale, RATIONALE_COLUMNS)
                    .into_iter()
                    .take(RATIONALE_ROWS)
                    .map(|line| (line, why)),
            );
        }
        blocks.push(lines);
    };
    if !correct {
        block(
            "-",
            picked,
            LessonTone::Misconception,
            LessonTone::MisconceptionWhy,
        );
    }
    block(
        "+",
        question.answer,
        LessonTone::Answer,
        LessonTone::AnswerWhy,
    );

    let line_count = blocks.iter().map(Vec::len).sum::<usize>() as i32;
    let height = line_count * LINE_HEIGHT - 1 + (blocks.len() as i32 - 1) * LESSON_BLOCK_GAP;
    let column_width = text_width(&"W".repeat(RATIONALE_COLUMNS), 1);
    let x = area.x + (area.width - column_width) / 2;
    let mut y = area.y + (area.height - height) / 2;
    let mut positioned = Vec::new();
    for block in blocks {
        for (text, tone) in block {
            positioned.push(LessonLine { x, y, text, tone });
            y += LINE_HEIGHT;
        }
        y += LESSON_BLOCK_GAP;
    }
    positioned
}

fn draw_lesson_lines(frame: &mut Framebuffer, lines: &[LessonLine]) {
    for line in lines {
        frame.text(line.x, line.y, &line.text, line.tone.color(), 1);
    }
}

/// The trial lesson panel's copy region above its footer row.
fn trial_lesson_copy_box() -> UiBox {
    UiBox {
        x: TRIAL_LESSON_BOX.x + 1,
        y: TRIAL_LESSON_BOX.y + 2,
        width: TRIAL_LESSON_BOX.width - 2,
        height: TRIAL_LESSON_BOX.height - 17,
    }
}

/// The trial lesson panel's footer row: the lens and its mastery runes on the
/// left, and the continue prompt on the right once input is live.
fn trial_lesson_footer_y() -> i32 {
    TRIAL_LESSON_BOX.y + TRIAL_LESSON_BOX.height - 11
}

/// The legacy lesson panel's copy region inside its border.
fn quiz_lesson_copy_box() -> UiBox {
    UiBox {
        x: QUIZ_LESSON_BOX.x + 1,
        y: QUIZ_LESSON_BOX.y + 1,
        width: QUIZ_LESSON_BOX.width - 2,
        height: QUIZ_LESSON_BOX.height - 2,
    }
}

/// The committed result's lesson copy, when the run is showing one.
fn current_lesson(state: &GameState, area: UiBox) -> Option<Vec<LessonLine>> {
    let run = state.quiz.as_ref()?;
    let (correct, _) = run.feedback?;
    let question = state.cartridge.as_ref()?.questions.get(run.question)?;
    let picked = run.source_choice(run.selected, question.choices.len());
    Some(lesson_lines(question, picked, correct, area))
}

fn render_quiz(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Trial) {
        render_oracle_trial(frame, state);
        return;
    }
    frame.clear(NAVY);
    let Some(run) = state.quiz.as_ref() else {
        return;
    };
    frame.rect(0, 0, WIDTH as i32, 16, INK);
    frame.text(5, 4, &format!("Q{:02}", run.question + 1), SKY, 1);
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
        frame.text(
            5,
            151,
            &quiz_feedback_banner(run),
            quiz_feedback_color(run),
            1,
        );
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

fn render_oracle_trial(frame: &mut Framebuffer, state: &GameState) {
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
    frame.text(
        44,
        7,
        &format!("TRIAL {:02}", (run.question + 1).min(99)),
        CYAN,
        1,
    );
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
    frame.text(
        126,
        20,
        &quiz_feedback_banner(run),
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
            let stage = cart
                .mastery
                .get(&concept)
                .map_or(0, |record| record.stage());
            frame.text(column_x, footer_y, concept.label(), CYAN_DIM, 1);
            draw_oracle_rune_meter(
                frame,
                column_x + text_width(concept.label(), 1) + 4,
                footer_y,
                stage,
                CYAN,
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

fn render_level_up(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Ascension) {
        render_oracle_ascension(frame, state);
        return;
    }
    frame.clear(NAVY);
    frame.outline(8, 8, 224, 144, PLUM);
    let pulse = ((state.screen_ticks / 10) % 3) as i32;
    draw_oracle_sigil(frame, 120, 72, pulse);
    frame.centered_text(22, "LEVEL UP!", GOLD, 2);
    let rise = (state.screen_ticks.min(45) / 5) as i32;
    draw_hero(frame, 106, 91 - rise, 1, state);
    if let Some(run) = state.quiz.as_ref() {
        frame.centered_text(116, &format!("LEVEL {}", run.level), PARCH, 1);
        frame.centered_text(
            130,
            &format!("BATCH {} SURVIVED", run.completed_batches),
            SKY,
            1,
        );
    }
    if state.screen_ticks >= LEVEL_UP_HOLD_TICKS {
        frame.centered_text(149, "A / START:CONTINUE", MIST, 1);
    } else {
        frame.centered_text(149, "ORACLE BOND DEEPENS", MIST, 1);
    }
}

fn render_oracle_ascension(frame: &mut Framebuffer, state: &GameState) {
    let tier = state.visual_tier();
    frame.blit_rgb(ORACLE_ASCENSION);
    frame.rect(
        ASCENSION_TITLE_BOX.x,
        ASCENSION_TITLE_BOX.y,
        ASCENSION_TITLE_BOX.width,
        ASCENSION_TITLE_BOX.height,
        VOID,
    );
    frame.outline(
        ASCENSION_TITLE_BOX.x,
        ASCENSION_TITLE_BOX.y,
        ASCENSION_TITLE_BOX.width,
        ASCENSION_TITLE_BOX.height,
        AMBER,
    );
    frame.centered_text_in(
        ASCENSION_TITLE_BOX.x,
        73,
        ASCENSION_TITLE_BOX.width,
        "ORACLE BOND ASCENDS",
        AMBER,
        1,
    );
    frame.centered_text_in(
        ASCENSION_TITLE_BOX.x,
        84,
        ASCENSION_TITLE_BOX.width,
        tier.label(),
        if tier == PresentationTier::OracleBound {
            MAGENTA
        } else {
            CYAN
        },
        2,
    );
    let rise = (state.screen_ticks.min(45) / 5) as i32;
    draw_hero(frame, 108, 105 - rise, 1, state);
    if let Some(run) = state.quiz.as_ref() {
        frame.rect(
            ASCENSION_LEVEL_BOX.x,
            ASCENSION_LEVEL_BOX.y,
            ASCENSION_LEVEL_BOX.width,
            ASCENSION_LEVEL_BOX.height,
            VOID,
        );
        frame.outline(
            ASCENSION_LEVEL_BOX.x,
            ASCENSION_LEVEL_BOX.y,
            ASCENSION_LEVEL_BOX.width,
            ASCENSION_LEVEL_BOX.height,
            CYAN_DIM,
        );
        frame.centered_text_box(
            ASCENSION_LEVEL_BOX,
            &format!("LEVEL {}", run.level.min(99)),
            PARCH,
            1,
        );
        frame.rect(
            ASCENSION_BATCH_BOX.x,
            ASCENSION_BATCH_BOX.y,
            ASCENSION_BATCH_BOX.width,
            ASCENSION_BATCH_BOX.height,
            VOID,
        );
        frame.outline(
            ASCENSION_BATCH_BOX.x,
            ASCENSION_BATCH_BOX.y,
            ASCENSION_BATCH_BOX.width,
            ASCENSION_BATCH_BOX.height,
            CYAN_DIM,
        );
        frame.centered_text_box(
            ASCENSION_BATCH_BOX,
            &format!("BATCH {}", run.completed_batches.min(99)),
            PARCH,
            1,
        );
    }
    frame.rect(
        MENU_FOOTER_BOX.x,
        MENU_FOOTER_BOX.y,
        MENU_FOOTER_BOX.width,
        MENU_FOOTER_BOX.height,
        VOID,
    );
    frame.outline(
        MENU_FOOTER_BOX.x,
        MENU_FOOTER_BOX.y,
        MENU_FOOTER_BOX.width,
        MENU_FOOTER_BOX.height,
        CYAN_DIM,
    );
    if state.screen_ticks >= LEVEL_UP_HOLD_TICKS {
        frame.centered_text_box(MENU_FOOTER_BOX, "A / START:CONTINUE", PARCH, 1);
    } else {
        frame.centered_text_box(MENU_FOOTER_BOX, "THE NEW CREST TAKES HOLD", MIST, 1);
    }
}

fn render_game_over(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Aftermath) {
        render_oracle_aftermath(frame, state);
        return;
    }
    frame.clear(INK);
    frame.outline(8, 8, 224, 144, PLUM);
    frame.centered_text(24, "GAME OVER", RED, 2);
    if let Some(run) = state.quiz.as_ref() {
        frame.centered_text(57, &format!("SCORE {:04}", run.score.min(9999)), GOLD, 1);
        frame.centered_text(
            83,
            &format!("INSIGHT {}", InsightStage::from_score(run.score).label()),
            InsightStage::from_score(run.score).color(),
            1,
        );
        frame.centered_text(70, &format!("LEVEL {} REACHED", run.level), SKY, 1);
        draw_oracle_sigil(frame, 120, 104, 0);
        draw_hero(frame, 106, 99, 1, state);
    } else {
        frame.centered_text(70, "NO QUESTIONS FOUND", GOLD, 1);
    }
    if (state.screen_ticks / 30).is_multiple_of(2) {
        frame.centered_text(143, "A/B/START:MENU", PARCH, 1);
    }
}

fn render_oracle_aftermath(frame: &mut Framebuffer, state: &GameState) {
    let tier = state.visual_tier();
    frame.blit_rgb(ORACLE_AFTERMATH);
    frame.centered_text_in(
        AFTERMATH_CONTENT_BOX.x,
        24,
        AFTERMATH_CONTENT_BOX.width,
        "VISION CLOSED",
        RED,
        1,
    );
    if let Some(run) = state.quiz.as_ref() {
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            49,
            AFTERMATH_CONTENT_BOX.width,
            "FINAL SCORE",
            MIST,
            1,
        );
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            59,
            AFTERMATH_CONTENT_BOX.width,
            &format!("{:04}", run.score.min(9999)),
            AMBER,
            1,
        );
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            76,
            AFTERMATH_CONTENT_BOX.width,
            "BOND REACHED",
            MIST,
            1,
        );
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            87,
            AFTERMATH_CONTENT_BOX.width,
            tier.label(),
            if tier == PresentationTier::Initiate {
                CYAN
            } else {
                AMBER
            },
            1,
        );
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            104,
            AFTERMATH_CONTENT_BOX.width,
            &format!("LEVEL {}", run.level.min(99)),
            PARCH,
            1,
        );
        let insight = InsightStage::from_score(run.score);
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            114,
            AFTERMATH_CONTENT_BOX.width,
            &format!("RUNE {}", insight.label()),
            insight.color(),
            1,
        );
        draw_defeated_hero(frame, 52, 106, state);
    } else {
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            79,
            AFTERMATH_CONTENT_BOX.width,
            "NO QUESTIONS",
            AMBER,
            1,
        );
    }
    if (state.screen_ticks / 30).is_multiple_of(2) {
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            126,
            AFTERMATH_CONTENT_BOX.width,
            "A/B/START:MENU",
            PARCH,
            1,
        );
    }
}

fn mastery_rune_color(stage: usize) -> Color {
    match stage {
        0 | 1 => CYAN,
        2 => AMBER,
        _ => MAGENTA,
    }
}

fn pending_reviews(lessons: &[Lesson], concept: Concept) -> usize {
    lessons
        .iter()
        .filter(|lesson| lesson.outstanding && lesson.concept == Some(concept))
        .count()
}

fn codex_lesson_counter(index: usize, total: usize) -> String {
    format!("LESSON {:02}/{:02}", (index + 1).min(999), total.min(999))
}

fn codex_lesson_status(lesson: &Lesson) -> (&'static str, Color) {
    if lesson.outstanding {
        ("REVIEW PENDING", AMBER)
    } else {
        ("LEARNED", CYAN)
    }
}

/// Learned and pending-review halves of the Codex totals line.
fn codex_totals(lessons: &[Lesson]) -> (String, String, Color) {
    let pending = lessons.iter().filter(|lesson| lesson.outstanding).count();
    let learned = format!("LEARNED {:02}", (lessons.len() - pending).min(99));
    if pending == 0 {
        (learned, "ALL CLEAR".into(), MIST)
    } else {
        (learned, format!("REVIEW {:02}", pending.min(99)), AMBER)
    }
}

fn draw_codex_totals(frame: &mut Framebuffer, bounds: UiBox, lessons: &[Lesson]) {
    let (learned, review, review_color) = codex_totals(lessons);
    let width = text_width(&format!("{learned}  {review}"), 1);
    let x = bounds.x + (bounds.width - width) / 2;
    let y = bounds.y + (bounds.height - 7) / 2;
    frame.text(x, y, &learned, CYAN, 1);
    let review_x = x + (learned.chars().count() as i32 + 2) * GLYPH_ADVANCE;
    frame.text(review_x, y, &review, review_color, 1);
}

fn draw_codex_panel(frame: &mut Framebuffer, bounds: UiBox) {
    frame.rect(bounds.x, bounds.y, bounds.width, bounds.height, VOID);
    frame.outline(bounds.x, bounds.y, bounds.width, bounds.height, CYAN_DIM);
}

/// Draws a lesson's lens label ending just left of its three mastery runes at
/// `runes_x`; a lesson without a lens shows `GENERAL` and no runes.
fn draw_codex_lens(
    frame: &mut Framebuffer,
    runes_x: i32,
    y: i32,
    concept: Option<Concept>,
    state: &GameState,
) {
    let Some(concept) = concept else {
        let label_x = runes_x + 19 - text_width("GENERAL", 1);
        frame.text(label_x, y, "GENERAL", MIST, 1);
        return;
    };
    let stage = state.mastery_stage(concept);
    let label_x = runes_x - 6 - text_width(concept.label(), 1);
    frame.text(label_x, y, concept.label(), PARCH, 1);
    draw_oracle_rune_meter(frame, runes_x, y, stage, mastery_rune_color(stage));
}

fn draw_codex_rationale(frame: &mut Framebuffer, x: i32, y: i32, lesson: &Lesson) {
    if lesson.rationale.trim().is_empty() {
        frame.text(x, y, "NO RATIONALE WAS RECORDED", MIST, 1);
    } else {
        frame.wrapped_text(
            x,
            y,
            &lesson.rationale,
            PARCH,
            RATIONALE_COLUMNS,
            RATIONALE_ROWS,
        );
    }
}

fn render_codex(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Codex) {
        render_oracle_codex(frame, state);
        return;
    }
    let Some((index, lesson)) = state.codex_lesson() else {
        render_codex_mastery(frame, state);
        return;
    };
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 16, INK);
    frame.text(
        5,
        4,
        &codex_lesson_counter(index, state.lessons().len()),
        SKY,
        1,
    );
    draw_codex_lens(frame, 214, 4, lesson.concept, state);
    let (status, status_color) = codex_lesson_status(lesson);
    frame.text(5, 21, status, status_color, 1);
    frame.rect(5, 31, 230, 38, INK);
    frame.outline(5, 31, 230, 38, SKY);
    frame.wrapped_text(
        11,
        35,
        &lesson.question,
        PARCH,
        QUIZ_QUESTION_COLUMNS,
        QUIZ_QUESTION_ROWS,
    );
    frame.rect(5, 72, 230, 15, INK);
    frame.text(9, 76, ">", GOLD, 1);
    frame.text(
        21,
        76,
        &truncate(&lesson.answer, QUIZ_CHOICE_CHARS),
        GREEN,
        1,
    );
    frame.rect(5, 90, 230, 52, INK);
    frame.outline(5, 90, 230, 52, MIST);
    frame.text(11, 95, "WHY IT HOLDS", SKY, 1);
    draw_codex_rationale(frame, 11, 107, lesson);
    frame.text(5, 151, "L/R:PAGE", MIST, 1);
    frame.text(199, 151, "B:BACK", MIST, 1);
}

fn render_codex_mastery(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 16, INK);
    frame.text(5, 4, "ORACLE CODEX", GOLD, 1);
    frame.text(195, 4, "MASTERY", SKY, 1);
    frame.outline(30, 24, 180, 82, SKY);
    let lessons = state.lessons();
    for (row, concept) in Concept::ALL.into_iter().enumerate() {
        let y = 32 + row as i32 * 14;
        let stage = state.mastery_stage(concept);
        frame.text(
            42,
            y,
            concept.label(),
            if stage > 0 { PARCH } else { MIST },
            1,
        );
        draw_oracle_rune_meter(frame, 128, y, stage, mastery_rune_color(stage));
        let pending = pending_reviews(lessons, concept);
        if pending > 0 {
            frame.text(156, y, &format!("!{}", pending.min(99)), AMBER, 1);
        }
    }
    if lessons.is_empty() {
        frame.centered_text(116, "NO LESSONS YET", GOLD, 1);
        frame.centered_text(130, "ANSWER TRIALS TO WRITE LESSONS", MIST, 1);
    } else {
        draw_codex_totals(
            frame,
            UiBox {
                x: 0,
                y: 116,
                width: WIDTH as i32,
                height: 7,
            },
            lessons,
        );
        frame.text(5, 151, "L/R:PAGE", MIST, 1);
    }
    frame.text(199, 151, "B:BACK", MIST, 1);
}

fn render_oracle_codex(frame: &mut Framebuffer, state: &GameState) {
    let Some((index, lesson)) = state.codex_lesson() else {
        render_oracle_codex_mastery(frame, state);
        return;
    };
    frame.blit_rgb(ORACLE_TRIAL);
    frame.blit_rgba(
        ORACLE_PORTRAITS[state.hero_style],
        HERO_PORTRAIT_SIZE,
        HERO_PORTRAIT_SIZE,
        8,
        5,
        1,
    );
    draw_codex_panel(frame, CODEX_LESSON_HEADER_BOX);
    frame.text(
        44,
        7,
        &codex_lesson_counter(index, state.lessons().len()),
        CYAN,
        1,
    );
    draw_codex_lens(frame, 214, 7, lesson.concept, state);
    let (status, status_color) = codex_lesson_status(lesson);
    frame.text(44, 20, status, status_color, 1);
    frame.text(147, 20, "L/R:PAGE B:BACK", MIST, 1);

    draw_codex_panel(frame, CODEX_QUESTION_BOX);
    frame.wrapped_text(
        CODEX_TEXT_X,
        CODEX_QUESTION_Y,
        &lesson.question,
        PARCH,
        QUIZ_QUESTION_COLUMNS,
        QUIZ_QUESTION_ROWS,
    );

    draw_codex_panel(frame, CODEX_ANSWER_BOX);
    draw_oracle_rune(frame, CODEX_TEXT_X, CODEX_ANSWER_Y, true, GREEN);
    frame.text(
        CODEX_ANSWER_TEXT_X,
        CODEX_ANSWER_Y,
        &truncate(&lesson.answer, QUIZ_CHOICE_CHARS),
        GREEN,
        1,
    );

    draw_codex_panel(frame, CODEX_RATIONALE_BOX);
    frame.text(CODEX_TEXT_X, CODEX_WHY_Y, "WHY IT HOLDS", CYAN_DIM, 1);
    draw_codex_rationale(frame, CODEX_TEXT_X, CODEX_RATIONALE_Y, lesson);
}

fn render_oracle_codex_mastery(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_CHRONICLE);
    frame.centered_text_box(CODEX_HEADING_BOX, "ORACLE CODEX", AMBER, 1);
    let lessons = state.lessons();
    for (row, concept) in Concept::ALL.into_iter().enumerate() {
        let y = CODEX_LENS_ROW_Y + row as i32 * CODEX_LENS_ROW_PITCH;
        let stage = state.mastery_stage(concept);
        frame.text(
            CODEX_LENS_LABEL_X,
            y,
            concept.label(),
            if stage > 0 { PARCH } else { MIST },
            1,
        );
        draw_oracle_rune_meter(
            frame,
            CODEX_LENS_RUNES_X,
            y,
            stage,
            mastery_rune_color(stage),
        );
        let pending = pending_reviews(lessons, concept);
        if pending > 0 {
            frame.text(
                CODEX_LENS_PENDING_X,
                y,
                &format!("!{}", pending.min(99)),
                AMBER,
                1,
            );
        }
    }
    draw_codex_panel(frame, CODEX_TOTALS_BOX);
    draw_codex_panel(frame, CODEX_PROMPT_BOX);
    if lessons.is_empty() {
        frame.centered_text_box(CODEX_TOTALS_BOX, "NO LESSONS YET", AMBER, 1);
        frame.centered_text_box(CODEX_PROMPT_BOX, "ANSWER A TRIAL  B:BACK", MIST, 1);
    } else {
        draw_codex_totals(frame, CODEX_TOTALS_BOX, lessons);
        frame.centered_text_box(CODEX_PROMPT_BOX, "L/R:READ LESSONS  B:BACK", MIST, 1);
    }
}

fn render_quest_select(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 22, INK);
    frame.centered_text(7, "CHOOSE THY QUEST", GOLD, 1);
    let Some(cart) = state.cartridge.as_ref() else {
        return;
    };
    if cart.quests.is_empty() {
        frame.centered_text(70, "NO QUESTS ON CARTRIDGE", RED, 1);
        return;
    }
    let start = state.quest_selected.saturating_sub(2);
    for (row, quest) in cart.quests.iter().skip(start).take(5).enumerate() {
        let index = start + row;
        let y = 31 + row as i32 * 22;
        if index == state.quest_selected {
            frame.rect(4, y - 3, 232, 18, ROYAL);
            frame.text(8, y, ">", GOLD, 1);
        }
        frame.text(20, y, &truncate(&quest.name, 35), PARCH, 1);
        frame.text(20, y + LINE_HEIGHT, &truncate(&quest.boss, 35), MIST, 1);
    }
    frame.centered_text(150, "A:FIGHT  B:BACK", MIST, 1);
}

fn render_battle(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 69, INK);
    frame.text(6, 5, &truncate(&state.active_boss, 37), RED, 1);
    draw_crab(frame, 34, 45, 1);
    draw_boss(frame, 183, 29, state.screen_ticks, 1);
    frame.rect(0, 68, WIDTH as i32, 2, SKY);
    frame.outline(4, 75, 232, 68, MIST);
    for (index, (line, stderr)) in state.logs.iter().rev().take(7).rev().enumerate() {
        frame.text(
            9,
            80 + index as i32 * LINE_HEIGHT,
            &truncate(line, 37),
            if *stderr { RED } else { PARCH },
            1,
        );
    }
    frame.text(5, 151, "B:ABORT", GOLD, 1);
    frame.text(164, 151, "RUST PROCESS", GREEN, 1);
}

fn render_result(frame: &mut Framebuffer, success: bool) {
    frame.clear(if success { NAVY } else { INK });
    frame.centered_text(
        39,
        if success { "QUEST" } else { "GAME" },
        if success { GOLD } else { RED },
        2,
    );
    frame.centered_text(
        60,
        if success { "CLEARED" } else { "OVER" },
        if success { GREEN } else { RED },
        2,
    );
    frame.outline(31, 91, 178, 33, if success { SKY } else { PLUM });
    frame.centered_text(103, "A:QUEST LIST", PARCH, 1);
}

fn draw_crab(frame: &mut Framebuffer, x: i32, y: i32, scale: i32) {
    frame.rect(x + 4 * scale, y, 20 * scale, 10 * scale, CRAB);
    frame.rect(x, y + 5 * scale, 28 * scale, 8 * scale, CRAB);
    frame.rect(x + 3 * scale, y + 13 * scale, 5 * scale, 4 * scale, CRAB);
    frame.rect(x + 20 * scale, y + 13 * scale, 5 * scale, 4 * scale, CRAB);
    frame.rect(x + 7 * scale, y + 3 * scale, 3 * scale, 3 * scale, INK);
    frame.rect(x + 18 * scale, y + 3 * scale, 3 * scale, 3 * scale, INK);
}

fn draw_hero(frame: &mut Framebuffer, x: i32, y: i32, scale: i32, state: &GameState) {
    if state.has_visual_template(VisualTemplate::Hero) {
        draw_oracle_hero(frame, x, y, scale, state);
        return;
    }
    let accent = HERO_STYLE_COLORS[state.hero_style];
    draw_crab(frame, x, y, scale);
    frame.rect(x + 8 * scale, y + 8 * scale, 12 * scale, 3 * scale, accent);
}

fn draw_oracle_hero(frame: &mut Framebuffer, x: i32, y: i32, scale: i32, state: &GameState) {
    frame.blit_rgba(
        ORACLE_HEROES[state.hero_style],
        HERO_SPRITE_WIDTH,
        HERO_SPRITE_HEIGHT,
        x,
        y,
        scale,
    );
}

fn draw_defeated_hero(frame: &mut Framebuffer, x: i32, y: i32, state: &GameState) {
    draw_hero(frame, x, y, 1, state);
    frame.rect(x + 5, y + 12, 14, 2, VOID);
    frame.line(x + 4, y + 35, x + 20, y + 35, ASH);
    frame.line(x - 7, y + 36, x + 31, y + 36, INDIGO);
}

fn draw_boss(frame: &mut Framebuffer, x: i32, y: i32, tick: u64, scale: i32) {
    let bob = ((tick / 15) % 2) as i32 * scale;
    frame.rect(x, y + bob, 30 * scale, 27 * scale, PLUM);
    frame.rect(
        x - 4 * scale,
        y + 7 * scale + bob,
        38 * scale,
        13 * scale,
        PLUM,
    );
    frame.rect(
        x + 5 * scale,
        y + 7 * scale + bob,
        5 * scale,
        5 * scale,
        GOLD,
    );
    frame.rect(
        x + 20 * scale,
        y + 7 * scale + bob,
        5 * scale,
        5 * scale,
        GOLD,
    );
    frame.rect(
        x + 8 * scale,
        y + 20 * scale + bob,
        14 * scale,
        3 * scale,
        RED,
    );
}

fn handle_effect(
    effect: EngineEffect,
    sender: &mpsc::Sender<EngineCommand>,
    child: &Arc<Mutex<Option<Child>>>,
    question_loader: &QuestionLoader,
    answered_question_recorder: &AnsweredQuestionRecorder,
) {
    match effect {
        EngineEffect::RunQuest(command) => run_quest(command, sender.clone(), Arc::clone(child)),
        EngineEffect::AbortQuest => {
            if let Ok(mut guard) = child.lock() {
                if let Some(process) = guard.as_mut() {
                    let _ = process.kill();
                }
            }
        }
        EngineEffect::RequestQuestions {
            cartridge_id,
            level,
            count,
        } => {
            let sender = sender.clone();
            let loader = Arc::clone(question_loader);
            thread::spawn(move || {
                let questions = loader(cartridge_id.clone(), level, count);
                let _ = sender.send(EngineCommand::Questions {
                    cartridge_id,
                    questions,
                });
            });
        }
        EngineEffect::RecordAnsweredQuestion {
            cartridge_id,
            evidence,
        } => answered_question_recorder(cartridge_id, evidence),
    }
}

fn run_quest(
    command: String,
    sender: mpsc::Sender<EngineCommand>,
    slot: Arc<Mutex<Option<Child>>>,
) {
    let mut guard = match slot.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if guard.is_some() {
        let _ = sender.send(EngineCommand::QuestOutput {
            line: "A QUEST IS ALREADY RUNNING".into(),
            stderr: true,
        });
        let _ = sender.send(EngineCommand::QuestDone { success: false });
        return;
    }
    let Some(mut shell) = external_tools::quest_shell_command() else {
        let _ = sender.send(EngineCommand::QuestOutput {
            line: "FAILED TO START: INSTALL GIT FOR WINDOWS OR SET CQA_SHELL".into(),
            stderr: true,
        });
        let _ = sender.send(EngineCommand::QuestDone { success: false });
        return;
    };
    let mut child = match shell
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = sender.send(EngineCommand::QuestOutput {
                line: format!("FAILED TO START: {error}"),
                stderr: true,
            });
            let _ = sender.send(EngineCommand::QuestDone { success: false });
            return;
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    *guard = Some(child);
    drop(guard);

    let out_sender = sender.clone();
    let out_reader = thread::spawn(move || {
        if let Some(stdout) = stdout {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = out_sender.send(EngineCommand::QuestOutput {
                    line,
                    stderr: false,
                });
            }
        }
    });
    let err_sender = sender.clone();
    let err_reader = thread::spawn(move || {
        if let Some(stderr) = stderr {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = err_sender.send(EngineCommand::QuestOutput { line, stderr: true });
            }
        }
    });
    thread::spawn(move || {
        let _ = out_reader.join();
        let _ = err_reader.join();
        let process = slot.lock().ok().and_then(|mut guard| guard.take());
        let success = process
            .and_then(|mut child| child.wait().ok())
            .is_some_and(|status| status.success());
        let _ = sender.send(EngineCommand::QuestDone { success });
    });
}

pub(crate) fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if word.chars().count() > max_chars {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let chars: Vec<char> = word.chars().collect();
            for chunk in chars.chunks(max_chars) {
                lines.push(chunk.iter().collect());
            }
        } else if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn text_width(text: &str, scale: i32) -> i32 {
    let characters = text.chars().count() as i32;
    let trailing_space = if characters > 0 { 1 } else { 0 };
    (characters * GLYPH_ADVANCE - trailing_space) * scale
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn title_lines(title: &str) -> Vec<String> {
    let words: Vec<&str> = title
        .split(|ch: char| ch == '-' || ch == '_' || ch.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect();
    let mut lines = vec![String::new()];
    for word in words {
        let candidate = if lines.last().is_some_and(|line| line.is_empty()) {
            word.to_string()
        } else {
            format!("{} {word}", lines.last().unwrap())
        };
        if candidate.chars().count() <= 19 {
            *lines.last_mut().unwrap() = candidate;
        } else if lines.len() == 1 {
            lines.push(truncate(word, 19));
        }
    }
    if lines[0].is_empty() {
        lines[0] = truncate(title, 19);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::MASTERY_THRESHOLDS;
    use crate::scene_machine::{
        SceneHandler, SceneMachineDefinition, SceneMachineTemplate, SceneSignal, SceneSpec,
        SceneTransition,
    };

    fn quiz_cartridge() -> CartridgeSpec {
        CartridgeSpec {
            id: "/tmp/engine-test".into(),
            title: "ENGINE TEST".into(),
            mode: CartridgeMode::Quiz,
            provenance: RepositoryProvenance {
                authors: vec!["ADA LOVELACE".into(), "GRACE HOPPER".into()],
                first_year: Some(2020),
                latest_year: Some(2024),
                copyright: Some("Copyright (c) 2020-2024 Ada Lovelace".into()),
            },
            codequest: None,
            machine: Box::new(SceneMachineDefinition::template(SceneMachineTemplate::Quiz)),
            quests: vec![],
            questions: vec![QuizQuestion {
                question: "WHO OWNS THE GAME LOOP?".into(),
                choices: vec!["BEVY".into(), "CSS".into(), "WEBKIT".into(), "HTML".into()],
                answer: 0,
                ..Default::default()
            }],
            question_batch_ends: Vec::new(),
            lessons: Vec::new(),
            mastery: Mastery::new(),
        }
    }

    fn oracle_template_cartridge() -> CartridgeSpec {
        let mut cartridge = quiz_cartridge();
        let config = CodeQuestConfig::parse(include_str!("../../CODEQUEST.toml"))
            .expect("the repository Oracle cartridge should parse");
        if let Some(title) = config.game.title.clone() {
            cartridge.title = title;
        }
        cartridge.machine = Box::new(
            config
                .runtime_machine()
                .expect("the Oracle scene graph should compile")
                .expect("schema v2 should produce a runtime machine"),
        );
        cartridge.codequest = Some(Box::new(config));
        cartridge
    }

    fn maybe_write_preview(name: &str, frame: &[u8]) {
        let Ok(directory) = std::env::var("CQA_VISUAL_PREVIEW_DIR") else {
            return;
        };
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).expect("preview directory should be writable");
        let mut ppm = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
        ppm.extend(
            frame
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|pixel| &pixel[..3])
                .copied(),
        );
        std::fs::write(directory.join(format!("{name}.ppm")), ppm)
            .expect("preview should be writable");
    }

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

    fn issue(engine: &mut GameEngine, command: EngineCommand) {
        engine.command(command);
        engine.update();
    }

    fn finish_opening(engine: &mut GameEngine) {
        issue(engine, EngineCommand::BootComplete);
        for _ in 0..59 {
            engine.update();
        }
        issue(
            engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        issue(
            engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: false,
            },
        );
        for _ in 0..89 {
            engine.update();
        }
        issue(
            engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: true,
            },
        );
        issue(
            engine,
            EngineCommand::Input {
                button: Button::Start,
                pressed: false,
            },
        );
        assert_eq!(engine.screen(), Screen::Title);
    }

    fn waiting_oracle_engine() -> GameEngine {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        let _ = engine.take_effects();
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        assert_eq!(engine.screen(), Screen::Oracle);
        engine
    }

    fn playing_quiz_engine() -> GameEngine {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        engine
    }

    fn press(engine: &mut GameEngine, button: Button) {
        issue(
            engine,
            EngineCommand::Input {
                button,
                pressed: true,
            },
        );
        issue(
            engine,
            EngineCommand::Input {
                button,
                pressed: false,
            },
        );
    }

    fn engine_state(engine: &GameEngine) -> &GameState {
        engine.app.world().resource::<GameState>()
    }

    fn current_question(engine: &GameEngine) -> QuizQuestion {
        let state = engine_state(engine);
        let run = state.quiz.as_ref().unwrap();
        state.cartridge.as_ref().unwrap().questions[run.question].clone()
    }

    /// The display slot that currently shows the correct answer.
    fn answer_slot(engine: &GameEngine) -> usize {
        let question = current_question(engine);
        let run = engine_state(engine).quiz.as_ref().unwrap();
        run.display_order(question.choices.len())
            .iter()
            .position(|source| *source == question.answer)
            .unwrap()
    }

    /// Moves focus to the answer slot, or to the slot after it for a miss.
    fn focus_choice(engine: &mut GameEngine, correct: bool) {
        let choice_count = current_question(engine).choices.len();
        let answer = answer_slot(engine);
        let target = if correct {
            answer
        } else {
            (answer + 1) % choice_count
        };
        while engine_state(engine).quiz.as_ref().unwrap().selected != target {
            press(engine, Button::Down);
        }
    }

    fn commit(engine: &mut GameEngine, correct: bool) {
        focus_choice(engine, correct);
        press(engine, Button::A);
    }

    /// Waits out the lesson hold and continues past the lesson card.
    fn finish_lesson(engine: &mut GameEngine) {
        for _ in 0..QUIZ_FEEDBACK_TICKS {
            engine.update();
        }
        press(engine, Button::A);
    }

    fn concept_question(index: usize) -> QuizQuestion {
        QuizQuestion {
            question: format!("WHICH LAYER OWNS CONCERN {index}?"),
            choices: vec![
                format!("THE ENGINE LAYER {index}"),
                format!("THE SHELL LAYER {index}"),
                format!("THE STYLE LAYER {index}"),
                format!("THE BUILD LAYER {index}"),
            ],
            answer: 0,
            concept: Some(Concept::Responsibility),
            rationales: vec![
                "THE ENGINE OWNS STATE AND RULES.".into(),
                "THE SHELL ONLY FORWARDS INPUT.".into(),
                "STYLES DRAW THE DEVICE CASE.".into(),
                "THE BUILD PACKAGES THE APP.".into(),
            ],
            review: false,
        }
    }

    /// A quiz engine playing one batch of distinct concept questions.
    fn batch_quiz_engine(count: usize) -> GameEngine {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        let _ = engine.take_effects();
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            press(&mut engine, button);
        }
        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: (0..count).map(concept_question).collect(),
            },
        );
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);
        engine
    }

    #[test]
    fn committing_an_answer_records_that_question_for_future_runs() {
        let mut engine = playing_quiz_engine();
        let _ = engine.take_effects();

        commit(&mut engine, true);

        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RecordAnsweredQuestion {
                cartridge_id,
                evidence,
            } if cartridge_id == "/tmp/engine-test"
                && evidence.question == "WHO OWNS THE GAME LOOP?"
                && evidence.correct
        )));
    }

    fn color_pixels_in_region(
        frame: &[u8],
        color: Color,
        x_range: std::ops::Range<usize>,
        y_range: std::ops::Range<usize>,
    ) -> usize {
        y_range
            .flat_map(|y| x_range.clone().map(move |x| (y * WIDTH + x) * 4))
            .filter(|&offset| frame[offset..offset + 4] == [color.0, color.1, color.2, 255])
            .count()
    }

    fn total_luminance(frame: &[u8]) -> u64 {
        frame
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[0] as u64 + pixel[1] as u64 + pixel[2] as u64)
            .sum()
    }

    #[derive(Clone, Copy, Debug)]
    struct LayoutBounds {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    fn text_bounds(x: i32, y: i32, text: &str, scale: i32) -> LayoutBounds {
        LayoutBounds {
            x,
            y,
            width: text_width(text, scale),
            height: 7 * scale,
        }
    }

    fn compact_text_bounds(x: i32, y: i32, text: &str) -> LayoutBounds {
        LayoutBounds {
            x,
            y,
            width: text.chars().count() as i32 * GLYPH_WIDTH,
            height: 7,
        }
    }

    fn alpha_bounds(rgba: &[u8], width: usize, height: usize) -> LayoutBounds {
        let mut min_x = width;
        let mut min_y = height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut found = false;
        for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
            if pixel[3] == 0 {
                continue;
            }
            let x = index % width;
            let y = index / width;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            found = true;
        }
        assert!(found, "sprite has no visible pixels");
        LayoutBounds {
            x: min_x as i32,
            y: min_y as i32,
            width: (max_x - min_x + 1) as i32,
            height: (max_y - min_y + 1) as i32,
        }
    }

    fn centered_text_bounds(y: i32, text: &str, scale: i32) -> LayoutBounds {
        let width = text_width(text, scale);
        text_bounds((WIDTH as i32 - width) / 2, y, text, scale)
    }

    fn centered_text_in_bounds(
        container: LayoutBounds,
        y: i32,
        text: &str,
        scale: i32,
    ) -> LayoutBounds {
        let width = text_width(text, scale);
        text_bounds(container.x + (container.width - width) / 2, y, text, scale)
    }

    fn ui_box_bounds(bounds: UiBox) -> LayoutBounds {
        LayoutBounds {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        }
    }

    fn centered_text_box_bounds(bounds: UiBox, text: &str, scale: i32) -> LayoutBounds {
        let width = text_width(text, scale);
        text_bounds(
            bounds.x + (bounds.width - width) / 2,
            bounds.y + (bounds.height - 7 * scale) / 2,
            text,
            scale,
        )
    }

    fn centered_compact_text_box_bounds(bounds: UiBox, text: &str) -> LayoutBounds {
        let width = text.chars().count() as i32 * GLYPH_WIDTH;
        compact_text_bounds(
            bounds.x + (bounds.width - width) / 2,
            bounds.y + (bounds.height - 7) / 2,
            text,
        )
    }

    fn bounds_contains(container: LayoutBounds, child: LayoutBounds) -> bool {
        child.x >= container.x
            && child.y >= container.y
            && child.x + child.width <= container.x + container.width
            && child.y + child.height <= container.y + container.height
    }

    fn bounds_are_disjoint(left: LayoutBounds, right: LayoutBounds) -> bool {
        left.x + left.width <= right.x
            || right.x + right.width <= left.x
            || left.y + left.height <= right.y
            || right.y + right.height <= left.y
    }

    fn horizontal_centers_align(container: LayoutBounds, child: LayoutBounds) -> bool {
        ((container.x * 2 + container.width) - (child.x * 2 + child.width)).abs() <= 1
    }

    fn vertical_centers_align(container: LayoutBounds, child: LayoutBounds) -> bool {
        ((container.y * 2 + container.height) - (child.y * 2 + child.height)).abs() <= 1
    }

    fn relative_luminance(color: Color) -> f64 {
        fn linear(channel: u8) -> f64 {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * linear(color.0) + 0.7152 * linear(color.1) + 0.0722 * linear(color.2)
    }

    fn contrast_ratio(foreground: Color, background: Color) -> f64 {
        let foreground = relative_luminance(foreground);
        let background = relative_luminance(background);
        let (lighter, darker) = if foreground > background {
            (foreground, background)
        } else {
            (background, foreground)
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn oracle_live_ui_stays_contained_disjoint_and_readable() {
        let screen = LayoutBounds {
            x: 0,
            y: 0,
            width: WIDTH as i32,
            height: HEIGHT as i32,
        };
        let chronicle_header = ui_box_bounds(CHRONICLE_HEADER_BOX);
        let chronicle_title = ui_box_bounds(CHRONICLE_TITLE_BOX);
        let chronicle_copyright = ui_box_bounds(CHRONICLE_COPYRIGHT_BOX);
        let chronicle_authors_label = ui_box_bounds(CHRONICLE_AUTHORS_LABEL_BOX);
        let chronicle_authors = ui_box_bounds(CHRONICLE_AUTHORS_BOX);
        let chronicle_footer = LayoutBounds {
            x: 42,
            y: 122,
            width: 156,
            height: 17,
        };
        let chronicle_skip = LayoutBounds {
            x: 74,
            y: 141,
            width: 92,
            height: 16,
        };
        let title_top = ui_box_bounds(GATEWAY_TITLE_TOP_BOX);
        let title_bottom = ui_box_bounds(GATEWAY_TITLE_BOTTOM_BOX);
        let title_prompt = ui_box_bounds(GATEWAY_PROMPT_BOX);
        let title_signature = ui_box_bounds(GATEWAY_SIGNATURE_BOX);
        let menu_option_text = ui_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1]);
        let menu_heading = ui_box_bounds(GATEWAY_MENU_HEADING_BOX);
        let menu_subtitle = ui_box_bounds(GATEWAY_MENU_SUBTITLE_BOX);
        let atelier_label = ui_box_bounds(atelier_label_box(ATELIER_ROW_BOXES[1]));
        let atelier_value = ui_box_bounds(atelier_value_box(ATELIER_ROW_BOXES[1]));
        let atelier_bind_text = ui_box_bounds(ATELIER_BIND_TEXT_BOX);
        let quiz_question = LayoutBounds {
            x: 20,
            y: 30,
            width: 200,
            height: 36,
        };
        let quiz_choice = LayoutBounds {
            x: 23,
            y: 69,
            width: 204,
            height: 20,
        };
        let aftermath_panel = ui_box_bounds(AFTERMATH_CONTENT_BOX);
        let opening_skip = LayoutBounds {
            x: 164,
            y: 149,
            width: 76,
            height: 11,
        };
        let menu_footer = ui_box_bounds(MENU_FOOTER_BOX);
        let atelier_header = ui_box_bounds(ATELIER_HEADER_BOX);
        let full_header = LayoutBounds {
            x: 0,
            y: 0,
            width: 240,
            height: 15,
        };
        let full_footer = LayoutBounds {
            x: 0,
            y: 143,
            width: 240,
            height: 17,
        };
        let ascension_title = ui_box_bounds(ASCENSION_TITLE_BOX);
        let trial_header = LayoutBounds {
            x: 37,
            y: 2,
            width: 203,
            height: 28,
        };
        let trial_ward = LayoutBounds {
            x: TRIAL_WARD_X,
            y: 7,
            width: 46,
            height: 7,
        };
        let trial_streak = text_bounds(TRIAL_STREAK_X, 7, "X3", 1);
        let trial_score_runes = LayoutBounds {
            x: TRIAL_SCORE_RUNES_X,
            y: 7,
            width: 19,
            height: 7,
        };
        let trial_score = text_bounds(TRIAL_SCORE_X, 7, "9999", 1);
        let ascension_level = ui_box_bounds(ASCENSION_LEVEL_BOX);
        let ascension_batch = ui_box_bounds(ASCENSION_BATCH_BOX);

        for (name, container, child) in [
            (
                "chronicle heading",
                chronicle_header,
                centered_compact_text_box_bounds(CHRONICLE_HEADER_BOX, "REPOSITORY CHRONICLE"),
            ),
            (
                "chronicle title first line",
                chronicle_title,
                compact_text_bounds(62, 49, "T".repeat(23).as_str()),
            ),
            (
                "chronicle title second line",
                chronicle_title,
                compact_text_bounds(62, 57, "T".repeat(23).as_str()),
            ),
            (
                "chronicle copyright first line",
                chronicle_copyright,
                compact_text_bounds(62, 67, "C".repeat(23).as_str()),
            ),
            (
                "chronicle copyright second line",
                chronicle_copyright,
                compact_text_bounds(62, 75, "C".repeat(23).as_str()),
            ),
            (
                "chronicle author heading",
                chronicle_authors_label,
                centered_compact_text_box_bounds(CHRONICLE_AUTHORS_LABEL_BOX, "COMMIT AUTHORS"),
            ),
            (
                "chronicle third author",
                chronicle_authors,
                compact_text_bounds(62, 110, "A".repeat(23).as_str()),
            ),
            (
                "chronicle history",
                chronicle_footer,
                centered_text_bounds(126, "ARCHIVE 1234 > 56789012", 1),
            ),
            (
                "chronicle skip",
                chronicle_skip,
                centered_text_bounds(145, "A / START:SKIP", 1),
            ),
            (
                "title first line",
                title_top,
                centered_text_box_bounds(GATEWAY_TITLE_TOP_BOX, "CODE QUEST", 2),
            ),
            (
                "title second line",
                title_bottom,
                centered_text_box_bounds(GATEWAY_TITLE_BOTTOM_BOX, "ADVANCE", 1),
            ),
            (
                "title prompt",
                title_prompt,
                centered_text_box_bounds(GATEWAY_PROMPT_BOX, "PRESS START", 1),
            ),
            (
                "title signature",
                title_signature,
                centered_text_box_bounds(GATEWAY_SIGNATURE_BOX, "REPOSITORY ORACLE", 1),
            ),
            (
                "awakening skip",
                opening_skip,
                text_bounds(166, 151, "A/START:SKIP", 1),
            ),
            (
                "menu option",
                menu_option_text,
                centered_text_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1], "RETURN TO TITLE", 1),
            ),
            (
                "menu heading",
                menu_heading,
                centered_text_box_bounds(GATEWAY_MENU_HEADING_BOX, "CHOOSE YOUR PATH", 1),
            ),
            (
                "menu subtitle",
                menu_subtitle,
                centered_text_box_bounds(GATEWAY_MENU_SUBTITLE_BOX, "THE BOND BEGINS HERE", 1),
            ),
            (
                "menu controls",
                menu_footer,
                centered_text_box_bounds(MENU_FOOTER_BOX, "D-PAD  A:CHOOSE  B:BACK", 1),
            ),
            (
                "atelier heading",
                atelier_header,
                centered_compact_text_box_bounds(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER"),
            ),
            (
                "atelier row label",
                atelier_label,
                centered_text_box_bounds(atelier_label_box(ATELIER_ROW_BOXES[1]), "PATH", 1),
            ),
            (
                "atelier longest value",
                atelier_value,
                centered_compact_text_box_bounds(
                    atelier_value_box(ATELIER_ROW_BOXES[1]),
                    "<MERGE PALADIN>",
                ),
            ),
            (
                "atelier bind action",
                atelier_bind_text,
                centered_text_box_bounds(ATELIER_BIND_TEXT_BOX, "BIND", 1),
            ),
            (
                "atelier retry status",
                full_footer,
                text_bounds(5, 148, "VISION CLOUDY - RETRYING", 1),
            ),
            (
                "atelier controls",
                full_footer,
                text_bounds(174, 148, "START:BIND", 1),
            ),
            (
                "sanctum status",
                full_header,
                text_bounds(152, 5, "CLAUDE:SCRYING", 1),
            ),
            (
                "sanctum controls",
                full_footer,
                centered_text_bounds(149, "L/R:MOVE  B:LEAVE", 1),
            ),
            (
                "quiz longest question line",
                quiz_question,
                text_bounds(28, 35, &"Q".repeat(QUIZ_QUESTION_COLUMNS), 1),
            ),
            (
                "trial number",
                trial_header,
                text_bounds(44, 7, "TRIAL 99", 1),
            ),
            ("trial ward", trial_header, trial_ward),
            ("trial streak multiplier", trial_header, trial_streak),
            ("trial score runes", trial_header, trial_score_runes),
            ("trial score", trial_header, trial_score),
            (
                "trial tier",
                trial_header,
                text_bounds(44, 20, "ORACLE-BOUND", 1),
            ),
            (
                "trial controls",
                trial_header,
                text_bounds(126, 20, "A:ANSWER B:LEAVE", 1),
            ),
            (
                "quiz longest choice",
                quiz_choice,
                text_bounds(TRIAL_CHOICE_TEXT_X, 76, &"C".repeat(QUIZ_CHOICE_CHARS), 1),
            ),
            (
                "ascension heading",
                ascension_title,
                centered_text_bounds(73, "ORACLE BOND ASCENDS", 1),
            ),
            (
                "ascension tier",
                ascension_title,
                centered_text_bounds(84, "ORACLE-BOUND", 2),
            ),
            (
                "ascension level",
                ascension_level,
                centered_text_box_bounds(ASCENSION_LEVEL_BOX, "LEVEL 99", 1),
            ),
            (
                "ascension batch",
                ascension_batch,
                centered_text_box_bounds(ASCENSION_BATCH_BOX, "BATCH 99", 1),
            ),
            (
                "ascension controls",
                menu_footer,
                centered_text_box_bounds(MENU_FOOTER_BOX, "A / START:CONTINUE", 1),
            ),
            (
                "aftermath tier",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, 87, "ORACLE-BOUND", 1),
            ),
            (
                "aftermath title",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, 24, "VISION CLOSED", 1),
            ),
            (
                "aftermath controls",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, 126, "A/B/START:MENU", 1),
            ),
            (
                "aftermath insight rune",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, 114, "RUNE III", 1),
            ),
        ] {
            assert!(
                bounds_contains(container, child),
                "{name} {child:?} exceeds its container {container:?}"
            );
            assert!(
                bounds_contains(screen, child),
                "{name} {child:?} exceeds the native frame"
            );
        }

        for (name, container, child) in [
            (
                "chronicle heading",
                chronicle_header,
                centered_compact_text_box_bounds(CHRONICLE_HEADER_BOX, "REPOSITORY CHRONICLE"),
            ),
            (
                "chronicle repository title",
                chronicle_title,
                centered_compact_text_box_bounds(CHRONICLE_TITLE_BOX, "CODE QUEST ADVANCE"),
            ),
            (
                "title first line",
                title_top,
                centered_text_box_bounds(GATEWAY_TITLE_TOP_BOX, "CODE QUEST", 2),
            ),
            (
                "title second line",
                title_bottom,
                centered_text_box_bounds(GATEWAY_TITLE_BOTTOM_BOX, "ADVANCE", 1),
            ),
            (
                "title prompt",
                title_prompt,
                centered_text_box_bounds(GATEWAY_PROMPT_BOX, "PRESS START", 1),
            ),
            (
                "title signature",
                title_signature,
                centered_text_box_bounds(GATEWAY_SIGNATURE_BOX, "REPOSITORY ORACLE", 1),
            ),
            (
                "menu heading",
                menu_heading,
                centered_text_box_bounds(GATEWAY_MENU_HEADING_BOX, "CHOOSE YOUR PATH", 1),
            ),
            (
                "menu subtitle",
                menu_subtitle,
                centered_text_box_bounds(GATEWAY_MENU_SUBTITLE_BOX, "THE BOND BEGINS HERE", 1),
            ),
            (
                "menu footer",
                menu_footer,
                centered_text_box_bounds(MENU_FOOTER_BOX, "D-PAD  A:CHOOSE  B:BACK", 1),
            ),
            (
                "menu option",
                menu_option_text,
                centered_text_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1], "RETURN TO TITLE", 1),
            ),
            (
                "atelier heading",
                atelier_header,
                centered_compact_text_box_bounds(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER"),
            ),
            (
                "atelier row label",
                atelier_label,
                centered_text_box_bounds(atelier_label_box(ATELIER_ROW_BOXES[1]), "PATH", 1),
            ),
            (
                "atelier row value",
                atelier_value,
                centered_compact_text_box_bounds(
                    atelier_value_box(ATELIER_ROW_BOXES[1]),
                    "<MERGE PALADIN>",
                ),
            ),
            (
                "atelier bind action",
                atelier_bind_text,
                centered_text_box_bounds(ATELIER_BIND_TEXT_BOX, "BIND", 1),
            ),
            (
                "ascension level",
                ascension_level,
                centered_text_box_bounds(ASCENSION_LEVEL_BOX, "LEVEL 99", 1),
            ),
            (
                "ascension batch",
                ascension_batch,
                centered_text_box_bounds(ASCENSION_BATCH_BOX, "BATCH 99", 1),
            ),
        ] {
            assert!(
                horizontal_centers_align(container, child),
                "{name} is not horizontally centered: {child:?} in {container:?}"
            );
            assert!(
                vertical_centers_align(container, child),
                "{name} is not vertically centered: {child:?} in {container:?}"
            );
        }

        for (name, container, child) in [
            (
                "ascension heading",
                ascension_title,
                centered_text_bounds(73, "ORACLE BOND ASCENDS", 1),
            ),
            (
                "ascension tier",
                ascension_title,
                centered_text_bounds(84, "ORACLE-BOUND", 2),
            ),
            (
                "aftermath title",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, 24, "VISION CLOSED", 1),
            ),
            (
                "aftermath score",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, 59, "9999", 1),
            ),
            (
                "aftermath controls",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, 126, "A/B/START:MENU", 1),
            ),
        ] {
            assert!(
                horizontal_centers_align(container, child),
                "{name} is not horizontally centered: {child:?} in {container:?}"
            );
        }

        for (name, left, right) in [
            (
                "atelier heading and name row",
                ui_box_bounds(ATELIER_HEADER_BOX),
                ui_box_bounds(ATELIER_ROW_BOXES[0]),
            ),
            (
                "atelier label and value",
                centered_text_box_bounds(atelier_label_box(ATELIER_ROW_BOXES[1]), "PATH", 1),
                centered_compact_text_box_bounds(
                    atelier_value_box(ATELIER_ROW_BOXES[1]),
                    "<MERGE PALADIN>",
                ),
            ),
            (
                "atelier status and controls",
                text_bounds(5, 148, "VISION CLOUDY - RETRYING", 1),
                text_bounds(174, 148, "START:BIND", 1),
            ),
            (
                "trial tier and controls",
                text_bounds(44, 20, "ORACLE-BOUND", 1),
                text_bounds(126, 20, "A:ANSWER B:LEAVE", 1),
            ),
            ("trial ward and streak", trial_ward, trial_streak),
            (
                "trial streak and score runes",
                trial_streak,
                trial_score_runes,
            ),
            (
                "trial score runes and score",
                trial_score_runes,
                trial_score,
            ),
            (
                "sanctum tier and status",
                text_bounds(60, 5, "ORACLE-BOUND", 1),
                text_bounds(152, 5, "CLAUDE:SCRYING", 1),
            ),
            (
                "ascension heading and tier",
                centered_text_bounds(73, "ORACLE BOND ASCENDS", 1),
                centered_text_bounds(84, "ORACLE-BOUND", 2),
            ),
            (
                "trial choice ornament and copy",
                LayoutBounds {
                    x: 23,
                    y: 69,
                    width: 14,
                    height: 20,
                },
                text_bounds(TRIAL_CHOICE_TEXT_X, 76, &"C".repeat(QUIZ_CHOICE_CHARS), 1),
            ),
        ] {
            assert!(
                bounds_are_disjoint(left, right),
                "{name} overlap: {left:?} and {right:?}"
            );
        }

        for (name, foreground) in [
            ("parchment", PARCH),
            ("mist", MIST),
            ("green", GREEN),
            ("red", RED),
            ("cyan", CYAN),
            ("secondary cyan", CYAN_DIM),
            ("amber", AMBER),
            ("magenta", MAGENTA),
        ] {
            let ratio = contrast_ratio(foreground, VOID);
            assert!(
                ratio >= 4.5,
                "{name} foreground contrast {ratio:.2}:1 is below 4.5:1"
            );
        }
    }

    #[test]
    fn atelier_hero_visible_feet_rest_on_the_stage_support_line() {
        const STAGE_SUPPORT_Y: i32 = 106;

        for hero in ORACLE_HEROES {
            let visible = alpha_bounds(hero, HERO_SPRITE_WIDTH, HERO_SPRITE_HEIGHT);
            let visible_bottom =
                ATELIER_HERO_Y + (visible.y + visible.height) * ATELIER_HERO_SCALE - 1;
            assert_eq!(
                visible_bottom,
                STAGE_SUPPORT_Y - 1,
                "the hero's visible feet must meet the atelier platform instead of sinking into it"
            );
        }
    }

    #[test]
    fn tracked_run_metrics_have_staged_thresholds_and_rewards() {
        assert_eq!(SCORE_RUNE_THRESHOLDS, [300, 900, 1_800]);
        assert_eq!(DATA_CHARGE_THRESHOLDS, [3, 6, 9]);
        assert_eq!(BUG_BREACH_THRESHOLDS, [1, 3, 5]);

        assert_eq!(streak_multiplier(0), 1);
        assert_eq!(streak_multiplier(2), 1);
        assert_eq!(streak_multiplier(3), 2);
        assert_eq!(streak_multiplier(4), 2);
        assert_eq!(streak_multiplier(5), 2);
        assert_eq!(streak_multiplier(6), 3);
        assert_eq!(streak_multiplier(7), 3);
        assert_eq!(score_award_for_streak(2), 100);
        assert_eq!(score_award_for_streak(3), 200);
        assert_eq!(score_award_for_streak(6), 300);

        assert_eq!(InsightStage::from_score(299), InsightStage::Unlit);
        assert_eq!(InsightStage::from_score(300), InsightStage::RuneOne);
        assert_eq!(InsightStage::from_score(301), InsightStage::RuneOne);
        assert_eq!(InsightStage::from_score(899), InsightStage::RuneOne);
        assert_eq!(InsightStage::from_score(900), InsightStage::RuneTwo);
        assert_eq!(InsightStage::from_score(901), InsightStage::RuneTwo);
        assert_eq!(InsightStage::from_score(1_799), InsightStage::RuneTwo);
        assert_eq!(InsightStage::from_score(1_800), InsightStage::RuneThree);
        assert_eq!(InsightStage::from_score(1_801), InsightStage::RuneThree);
        assert_eq!(threshold_stage(2, &DATA_CHARGE_THRESHOLDS), 0);
        assert_eq!(threshold_stage(3, &DATA_CHARGE_THRESHOLDS), 1);
        assert_eq!(threshold_stage(4, &DATA_CHARGE_THRESHOLDS), 1);
        assert_eq!(threshold_stage(5, &DATA_CHARGE_THRESHOLDS), 1);
        assert_eq!(threshold_stage(6, &DATA_CHARGE_THRESHOLDS), 2);
        assert_eq!(threshold_stage(7, &DATA_CHARGE_THRESHOLDS), 2);
        assert_eq!(threshold_stage(8, &DATA_CHARGE_THRESHOLDS), 2);
        assert_eq!(threshold_stage(9, &DATA_CHARGE_THRESHOLDS), 3);
        assert_eq!(threshold_stage(10, &DATA_CHARGE_THRESHOLDS), 3);
        assert_eq!(threshold_stage(0, &BUG_BREACH_THRESHOLDS), 0);
        assert_eq!(threshold_stage(1, &BUG_BREACH_THRESHOLDS), 1);
        assert_eq!(threshold_stage(2, &BUG_BREACH_THRESHOLDS), 1);
        assert_eq!(threshold_stage(3, &BUG_BREACH_THRESHOLDS), 2);
        assert_eq!(threshold_stage(4, &BUG_BREACH_THRESHOLDS), 2);
        assert_eq!(threshold_stage(5, &BUG_BREACH_THRESHOLDS), 3);
        assert_eq!(threshold_stage(6, &BUG_BREACH_THRESHOLDS), 3);
    }

    #[test]
    fn correct_answers_apply_the_staged_flow_multiplier_to_runtime_score() {
        let mut engine = playing_quiz_engine();
        let expected_scores = [100, 200, 400, 600, 800, 1_100, 1_400, 1_700, 2_000];
        focus_choice(&mut engine, true);

        for (index, expected_score) in expected_scores.into_iter().enumerate() {
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
            let state = engine.app.world().resource::<GameState>();
            let run = state.quiz.as_ref().unwrap();
            assert_eq!(run.streak, index as u32 + 1);
            assert_eq!(run.score, expected_score);
            assert_eq!(
                InsightStage::from_score(run.score).index(),
                [0, 0, 1, 1, 1, 2, 2, 2, 3][index]
            );
            engine
                .app
                .world_mut()
                .resource_mut::<GameState>()
                .quiz
                .as_mut()
                .unwrap()
                .feedback = None;
        }
    }

    #[test]
    fn quiz_review_names_rune_flow_and_ward_threshold_changes() {
        let mut run = QuizRun {
            score: 400,
            streak: 3,
            feedback: Some((true, QUIZ_FEEDBACK_TICKS)),
            ..QuizRun::new()
        };
        assert_eq!(quiz_feedback_banner(&run), "RUNE I AWAKENS");

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
    fn oracle_ward_runes_show_each_health_threshold_without_generic_pips() {
        let mut full = Framebuffer::default();
        let mut strained = Framebuffer::default();
        let mut fractured = Framebuffer::default();
        let mut broken = Framebuffer::default();
        draw_oracle_ward_meter(&mut full, 0, 0, 3);
        draw_oracle_ward_meter(&mut strained, 0, 0, 2);
        draw_oracle_ward_meter(&mut fractured, 0, 0, 1);
        draw_oracle_ward_meter(&mut broken, 0, 0, 0);

        assert_ne!(full.pixels, strained.pixels);
        assert_ne!(strained.pixels, fractured.pixels);
        assert_ne!(fractured.pixels, broken.pixels);
        assert!(color_pixels_in_region(&full.pixels, CYAN, 0..46, 0..7) > 0);
        assert!(color_pixels_in_region(&strained.pixels, AMBER, 0..46, 0..7) > 0);
        assert!(color_pixels_in_region(&fractured.pixels, RED, 0..46, 0..7) > 0);
    }

    #[test]
    fn threshold_crossings_change_datafall_runes_at_the_declared_breakpoints() {
        let mut state = GameState {
            cartridge: Some(oracle_template_cartridge()),
            questions_loading: true,
            oracle_data: 2,
            oracle_bug_hits: 0,
            ..Default::default()
        };
        let mut before = Framebuffer::default();
        render_oracle_sanctum(&mut before, &state);

        state.oracle_data = 3;
        state.oracle_bug_hits = 1;
        let mut after = Framebuffer::default();
        render_oracle_sanctum(&mut after, &state);

        assert_ne!(
            frame_region(&before.pixels, 49..68, 149..156),
            frame_region(&after.pixels, 49..68, 149..156),
            "the first data charge threshold must light a rune"
        );
        assert_ne!(
            frame_region(&before.pixels, 173..192, 149..156),
            frame_region(&after.pixels, 173..192, 149..156),
            "the first corruption threshold must break a containment rune"
        );
    }

    fn frame_region(
        frame: &[u8],
        x_range: std::ops::Range<usize>,
        y_range: std::ops::Range<usize>,
    ) -> Vec<u8> {
        y_range
            .flat_map(|y| {
                x_range.clone().flat_map(move |x| {
                    let offset = (y * WIDTH + x) * 4;
                    frame[offset..offset + 4].iter().copied()
                })
            })
            .collect()
    }

    fn hero_pixels(engine: &GameEngine) -> Vec<u8> {
        let frame = engine.frame();
        (40..92)
            .flat_map(|y| {
                (8..64).flat_map(move |x| {
                    let offset = (y * WIDTH + x) * 4;
                    frame[offset..offset + 4].iter().copied()
                })
            })
            .collect()
    }

    #[test]
    fn framebuffer_is_always_fixed_resolution() {
        let mut engine = GameEngine::new();
        assert_eq!((WIDTH, HEIGHT), (240, 160));
        assert_eq!(FRAME_BYTES, 153_600);
        assert_eq!(engine.frame().len(), FRAME_BYTES);
        assert!(engine
            .frame()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255));
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        for button in [
            Button::Up,
            Button::Down,
            Button::Left,
            Button::Right,
            Button::A,
            Button::B,
            Button::Start,
            Button::Select,
            Button::L,
            Button::R,
        ] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            assert_eq!(
                engine.frame().len(),
                FRAME_BYTES,
                "{button:?} changed the framebuffer size"
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
            assert_eq!(
                engine.frame().len(),
                FRAME_BYTES,
                "releasing {button:?} changed the framebuffer size"
            );
        }
    }

    #[test]
    fn oracle_gameplay_cannot_change_resolution() {
        let mut engine = waiting_oracle_engine();
        for button in [Button::Left, Button::Right, Button::Up, Button::Down] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            for _ in 0..30 {
                engine.update();
                assert_eq!(engine.frame().len(), FRAME_BYTES);
            }
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
    }

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
    fn oracle_hud_separates_oracle_info_at_top_from_game_info_at_bottom() {
        let engine = waiting_oracle_engine();
        let frame = engine.frame();
        for (label, color) in [("Oracle title/progress", GOLD), ("AI provider status", SKY)] {
            assert!(
                color_pixels_in_region(frame, color, 0..WIDTH, 0..12) > 0,
                "{label} is missing from the top quiz HUD"
            );
        }
        for (label, color) in [
            ("data counter", GREEN),
            ("controls", PARCH),
            ("bug counter", RED),
        ] {
            assert!(
                color_pixels_in_region(frame, color, 0..WIDTH, 132..HEIGHT) > 0,
                "{label} is missing from the bottom game HUD"
            );
        }
        assert_eq!(color_pixels_in_region(frame, GREEN, 0..WIDTH, 0..12), 0);
        assert_eq!(color_pixels_in_region(frame, RED, 0..WIDTH, 0..12), 0);
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
        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: vec![QuizQuestion {
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
            },
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
    fn late_oracle_result_cannot_mutate_a_different_cartridge() {
        let mut engine = GameEngine::new();
        let first = quiz_cartridge();
        let mut second = quiz_cartridge();
        second.id = "/tmp/second".into();
        second.questions[0].question = "SECOND CARTRIDGE".into();
        issue(&mut engine, EngineCommand::Cartridge(Some(first)));
        issue(&mut engine, EngineCommand::Cartridge(Some(second)));
        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: vec![QuizQuestion {
                    question: "STALE".into(),
                    choices: vec!["A".into()],
                    answer: 0,
                    ..Default::default()
                }],
            },
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

        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: vec![QuizQuestion {
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
            },
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

        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: vec![QuizQuestion {
                    question: "WHAT ARRIVED NEXT?".into(),
                    choices: vec!["A NEW QUESTION".into(), "NOTHING".into()],
                    answer: 0,
                    ..Default::default()
                }],
            },
        );
        assert_eq!(engine.screen(), Screen::Quiz);
        let state = engine.app.world().resource::<GameState>();
        assert_eq!(
            state.cartridge.as_ref().unwrap().questions[1].question,
            "WHAT ARRIVED NEXT?"
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
    fn identity_choices_preserve_authored_hero_art_while_aura_changes_colorway() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        assert_eq!(engine.screen(), Screen::CharacterCreation);

        let default_name = hero_pixels(&engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_eq!(hero_pixels(&engine), default_name);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: false,
            },
        );

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: true,
            },
        );
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: false,
            },
        );
        let default_path = hero_pixels(&engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_eq!(hero_pixels(&engine), default_path);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: false,
            },
        );

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: true,
            },
        );
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: false,
            },
        );
        let default_style = hero_pixels(&engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_ne!(hero_pixels(&engine), default_style);
    }

    #[test]
    fn empty_quiz_cartridge_requests_the_first_ai_batch() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));

        let effects = engine.take_effects();
        assert!(matches!(
            effects.as_slice(),
            [EngineEffect::RequestQuestions {
                cartridge_id,
                level: 1,
                count: 6,
            }] if cartridge_id == "/tmp/engine-test"
        ));
    }

    #[test]
    fn oracle_retries_a_failed_ai_batch_while_waiting() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        let _ = engine.take_effects();
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        assert_eq!(engine.screen(), Screen::Oracle);
        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: Vec::new(),
            },
        );

        for _ in 0..300 {
            engine.update();
        }

        assert_eq!(engine.screen(), Screen::Oracle);
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RequestQuestions {
                cartridge_id,
                level: 1,
                count: 6,
            } if cartridge_id == "/tmp/engine-test"
        )));
    }

    #[test]
    fn failed_prefetch_waits_before_requesting_ai_again() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = vec![cartridge.questions[0].clone(); 4];
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        let _ = engine.take_effects();
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        for _ in 0..75 {
            engine.update();
        }
        assert!(matches!(
            engine.take_effects().as_slice(),
            [EngineEffect::RequestQuestions { level: 2, .. }]
        ));

        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: Vec::new(),
            },
        );
        assert!(engine.take_effects().is_empty());

        for _ in 0..299 {
            engine.update();
        }
        assert!(engine.take_effects().is_empty());
        engine.update();
        assert!(matches!(
            engine.take_effects().as_slice(),
            [EngineEffect::RequestQuestions { level: 2, .. }]
        ));
    }

    #[test]
    fn next_batch_prefetch_starts_when_the_first_batch_becomes_playable() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = vec![cartridge.questions[0].clone(); 6];
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        let _ = engine.take_effects();
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        for _ in 0..75 {
            engine.update();
        }

        assert!(matches!(
            engine.take_effects().as_slice(),
            [EngineEffect::RequestQuestions {
                level: 2,
                count: 6,
                ..
            }]
        ));
    }

    #[test]
    fn surviving_a_complete_ai_batch_levels_up() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);

        // Two misses add two review copies, so the batch closes after eight
        // commitments: the six originals plus both retries.
        let batch_length = QUESTION_BATCH_SIZE + 2;
        for (index, wrong) in [true, false, false, true, false, false, false, false]
            .into_iter()
            .enumerate()
        {
            commit(&mut engine, !wrong);
            finish_lesson(&mut engine);
            if index + 1 < batch_length {
                assert_eq!(engine.screen(), Screen::Quiz, "commitment {index}");
            }
        }

        assert_eq!(engine.screen(), Screen::LevelUp);
        let state = engine_state(&engine);
        let run = state.quiz.as_ref().unwrap();
        assert_eq!(run.level, 2);
        assert_eq!(run.hearts, 1);
        assert_eq!(run.question, batch_length);
        assert_eq!(state.batch_ends, vec![batch_length]);
    }

    #[test]
    fn losing_the_last_heart_at_a_batch_boundary_does_not_level_up() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        let question = cartridge.questions[0].clone();
        cartridge.questions.clear();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        let _ = engine.take_effects();
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: vec![question; 3],
            },
        );
        for _ in 0..75 {
            engine.update();
        }

        for _ in 0..3 {
            commit(&mut engine, false);
            finish_lesson(&mut engine);
        }

        assert_eq!(engine.screen(), Screen::GameOver);
        let state = engine.app.world().resource::<GameState>();
        let run = state.quiz.as_ref().unwrap();
        assert_eq!(run.level, 1);
        assert_eq!(run.completed_batches, 0);
        // The two survivable misses were scheduled for review; the final,
        // ward-breaking miss was not.
        assert_eq!(state.cartridge.as_ref().unwrap().questions.len(), 5);
        assert_eq!(state.batch_ends, vec![5]);
    }

    #[test]
    fn four_correct_answers_do_not_level_up_before_the_batch_ends() {
        let mut engine = GameEngine::new();
        let mut cartridge = quiz_cartridge();
        cartridge.questions = vec![cartridge.questions[0].clone(); 6];
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A, Button::Start] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);

        for _ in 0..4 {
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }

        assert_eq!(engine.screen(), Screen::Quiz);
        let state = engine.app.world().resource::<GameState>();
        let run = state.quiz.as_ref().unwrap();
        assert_eq!(run.level, 1);
        assert_eq!(run.completed_batches, 0);
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RequestQuestions {
                cartridge_id,
                level: 2,
                count: 6,
            } if cartridge_id == "/tmp/engine-test"
        )));
    }

    #[test]
    fn question_identity_matches_the_save_normalization() {
        assert_eq!(
            question_identity("  who owns\tthe\n game   loop? "),
            "WHO OWNS THE GAME LOOP?"
        );
        assert_eq!(
            question_identity("WHO OWNS THE GAME LOOP?"),
            question_identity("who owns the game loop?")
        );
    }

    #[test]
    fn committing_maps_the_focused_display_slot_to_its_source_choice() {
        for correct in [true, false] {
            let mut engine = batch_quiz_engine(1);
            let _ = engine.take_effects();
            let question = current_question(&engine);
            let slot = answer_slot(&engine);
            {
                let run = engine_state(&engine).quiz.as_ref().unwrap();
                let mut order = run.order.clone();
                assert_eq!(run.display_order(4)[slot], question.answer);
                order.sort_unstable();
                assert_eq!(order, [0, 1, 2, 3], "the display order is a permutation");
            }

            commit(&mut engine, correct);

            let run = engine_state(&engine).quiz.as_ref().unwrap();
            assert_eq!(run.feedback.map(|(result, _)| result), Some(correct));
            assert!(engine.take_effects().iter().any(|effect| matches!(
                effect,
                EngineEffect::RecordAnsweredQuestion { evidence, .. }
                    if evidence.correct == correct
                        && evidence.concept == Some(Concept::Responsibility)
                        && !evidence.review
            )));
        }
    }

    #[test]
    fn provider_first_answers_spread_across_every_display_slot() {
        let mut cartridge = quiz_cartridge();
        cartridge.questions = (0..60).map(concept_question).collect();
        let mut state = GameState {
            cartridge: Some(cartridge),
            quiz: Some(QuizRun::new()),
            ..Default::default()
        };
        let mut slot_counts = [0; 4];
        for index in 0..60 {
            state.quiz.as_mut().unwrap().question = index;
            present_current_question(&mut state);
            let run = state.quiz.as_ref().unwrap();
            assert_eq!(run.presented, Some(index));
            assert_eq!(run.selected, 0);
            let slot = run.order.iter().position(|source| *source == 0).unwrap();
            slot_counts[slot] += 1;
        }
        assert!(
            slot_counts.iter().all(|count| *count >= 5),
            "the answer must not favor a slot: {slot_counts:?}"
        );
    }

    #[test]
    fn retry_insertion_respects_the_gap_and_the_batch_end() {
        assert_eq!(retry_insertion_index(1, Some(6), 12), 5);
        assert_eq!(retry_insertion_index(4, Some(6), 12), 6);
        assert_eq!(retry_insertion_index(5, Some(6), 12), 6);
        assert_eq!(retry_insertion_index(0, None, 1), 1);

        let mut questions: Vec<_> = (0..12).map(concept_question).collect();
        let mut batch_ends = vec![6, 12];

        // Mid-batch: three other questions separate the miss from its review.
        assert_eq!(schedule_retry(&mut questions, &mut batch_ends, 1), Some(5));
        assert_eq!(batch_ends, vec![7, 13]);
        assert_eq!(questions[5].question, questions[1].question);
        assert!(questions[5].review);
        assert!(!questions[1].review);

        // Near the batch end: the review becomes the batch's last question.
        assert_eq!(schedule_retry(&mut questions, &mut batch_ends, 5), Some(7));
        assert_eq!(batch_ends, vec![8, 14]);
        assert_eq!(questions[7].question, questions[5].question);

        // At the batch end itself: the review follows immediately.
        assert_eq!(schedule_retry(&mut questions, &mut batch_ends, 7), Some(8));
        assert_eq!(batch_ends, vec![9, 15]);
        assert_eq!(questions.len(), 15);
        assert_eq!(questions[9].question, concept_question(6).question);
        assert_eq!(schedule_retry(&mut questions, &mut batch_ends, 99), None);
    }

    #[test]
    fn a_missed_question_returns_in_a_new_slot_and_redeems_its_lesson() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        let missed = current_question(&engine);
        let first_slot = answer_slot(&engine);

        commit(&mut engine, false);
        {
            let state = engine_state(&engine);
            let cartridge = state.cartridge.as_ref().unwrap();
            assert_eq!(cartridge.questions.len(), QUESTION_BATCH_SIZE + 1);
            assert_eq!(state.batch_ends, vec![QUESTION_BATCH_SIZE + 1]);
            assert_eq!(
                cartridge.lessons,
                vec![Lesson {
                    question: missed.question.clone(),
                    answer: missed.choices[0].clone(),
                    rationale: missed.rationales[0].clone(),
                    concept: Some(Concept::Responsibility),
                    outstanding: true,
                }]
            );
            assert_eq!(cartridge.mastery[&Concept::Responsibility].missed, 1);
        }
        finish_lesson(&mut engine);

        for _ in 0..RETRY_GAP {
            assert!(!current_question(&engine).review);
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }

        let review = current_question(&engine);
        assert!(review.review);
        assert_eq!(review.question, missed.question);
        assert_ne!(
            answer_slot(&engine),
            first_slot,
            "a retry must move the answer so position cannot be memorized"
        );
        assert_eq!(engine_state(&engine).quiz.as_ref().unwrap().attempt, 1);

        let _ = engine.take_effects();
        commit(&mut engine, true);
        let state = engine_state(&engine);
        let run = state.quiz.as_ref().unwrap();
        assert!(run.redeemed);
        assert_eq!(quiz_feedback_banner(run), "REDEEMED");
        let cartridge = state.cartridge.as_ref().unwrap();
        assert_eq!(cartridge.lessons.len(), 1 + RETRY_GAP);
        assert!(
            !cartridge.lessons[0].outstanding,
            "redemption clears the miss"
        );
        assert_eq!(
            cartridge.mastery[&Concept::Responsibility],
            learning::LensRecord {
                first_try: RETRY_GAP as u32,
                redeemed: 1,
                missed: 1,
            }
        );
        assert!(engine.take_effects().iter().any(|effect| matches!(
            effect,
            EngineEffect::RecordAnsweredQuestion { evidence, .. }
                if evidence.correct && evidence.review
        )));
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
        assert_eq!(quiz_feedback_banner(&run), "RUNE I AWAKENS");
        run.score = 400;
        assert_eq!(quiz_feedback_banner(&run), "REDEEMED");
        assert_eq!(quiz_feedback_color(&run).1, GREEN.1);
        run.redeemed = false;
        assert_eq!(quiz_feedback_banner(&run), "REVIEW ANSWER");
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

    fn lesson_question() -> QuizQuestion {
        QuizQuestion {
            question: "WHY SEPARATE GAME STATE FROM THE DEVICE SHELL?".into(),
            choices: vec![
                "TO KEEP RESPONSIBILITIES CLEAR".into(),
                "TO DUPLICATE RUNTIME STATE".into(),
                "TO HIDE INPUT TRANSITIONS".into(),
                "TO COUPLE RENDERING TO CSS".into(),
            ],
            answer: 0,
            concept: Some(Concept::Responsibility),
            rationales: vec![
                "THE ENGINE OWNS RULES AND TICKS, SO THE SHELL ONLY DRAWS FRAMES AND FORWARDS INPUT."
                    .into(),
                "ONE OWNER KEEPS STATE TRUE; A SECOND COPY IN THE SHELL WOULD DRIFT FROM THE ENGINE."
                    .into(),
                "INPUT EDGES ARE EXPLICIT ENGINE EVENTS; THE SPLIT DOES NOT HIDE THEM.".into(),
                "THE ENGINE RENDERS CPU PIXELS; CSS ONLY STYLES THE DEVICE CASE.".into(),
            ],
            review: false,
        }
    }

    /// 31-character choices and rationales that fill all three 34-column rows.
    fn worst_case_lesson_question() -> QuizQuestion {
        let row = format!("{} {}", "W".repeat(16), "W".repeat(17));
        QuizQuestion {
            question: "Q".repeat(QUIZ_QUESTION_COLUMNS),
            choices: ["A", "B", "C", "D"]
                .map(|letter| letter.repeat(QUIZ_CHOICE_CHARS))
                .to_vec(),
            answer: 0,
            concept: Some(Concept::Invariant),
            rationales: vec![[row.as_str(), row.as_str(), row.as_str()].join(" "); 4],
            review: false,
        }
    }

    fn lesson_state(question: QuizQuestion, correct: bool, live: bool) -> GameState {
        let mut cartridge = oracle_template_cartridge();
        cartridge.questions = vec![question];
        cartridge.mastery.insert(
            Concept::Responsibility,
            learning::LensRecord {
                first_try: 3,
                ..Default::default()
            },
        );
        GameState {
            cartridge: Some(cartridge),
            screen: Screen::Quiz,
            quiz: Some(QuizRun {
                hearts: if correct { 3 } else { 2 },
                score: if correct { 200 } else { 100 },
                streak: if correct { 2 } else { 0 },
                selected: if correct { 0 } else { 1 },
                feedback: Some((correct, if live { 0 } else { QUIZ_FEEDBACK_TICKS })),
                ..QuizRun::new()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn lesson_panel_copy_stays_contained_disjoint_and_readable() {
        let question = worst_case_lesson_question();
        assert!(question.rationales.iter().all(|rationale| wrap_text(
            rationale,
            RATIONALE_COLUMNS
        )
        .len()
            == RATIONALE_ROWS
            && learning::rationale_fits(rationale)));
        let panel = ui_box_bounds(TRIAL_LESSON_BOX);
        let interior = LayoutBounds {
            x: panel.x + 1,
            y: panel.y + 1,
            width: panel.width - 2,
            height: panel.height - 2,
        };
        let screen = LayoutBounds {
            x: 0,
            y: 0,
            width: WIDTH as i32,
            height: HEIGHT as i32,
        };
        let question_copy = text_bounds(
            28,
            35 + (QUIZ_QUESTION_ROWS as i32 - 1) * LINE_HEIGHT,
            &"Q".repeat(QUIZ_QUESTION_COLUMNS),
            1,
        );
        assert!(bounds_contains(screen, panel));
        assert!(bounds_are_disjoint(panel, question_copy));

        for (name, correct, expected_lines) in [("missed", false, 8), ("correct", true, 4)] {
            let lines = lesson_lines(&question, 1, correct, trial_lesson_copy_box());
            assert_eq!(lines.len(), expected_lines, "{name}");
            let column_x = lines[0].x;
            let column_end = column_x + text_width(&"W".repeat(RATIONALE_COLUMNS), 1);
            let footer_y = trial_lesson_footer_y();
            let label = text_bounds(column_x, footer_y, Concept::Invariant.label(), 1);
            let runes = LayoutBounds {
                x: label.x + label.width + 4,
                y: footer_y,
                width: 19,
                height: 7,
            };
            let prompt = text_bounds(
                column_end - text_width(LESSON_CONTINUE, 1),
                footer_y,
                LESSON_CONTINUE,
                1,
            );
            let rule = LayoutBounds {
                x: column_x,
                y: footer_y - 3,
                width: column_end - column_x,
                height: 1,
            };
            let mut children: Vec<(String, LayoutBounds)> = lines
                .iter()
                .map(|line| {
                    (
                        line.text.clone(),
                        text_bounds(line.x, line.y, &line.text, 1),
                    )
                })
                .collect();
            children.extend([
                ("lens label".to_string(), label),
                ("lens runes".to_string(), runes),
                ("continue prompt".to_string(), prompt),
                ("footer rule".to_string(), rule),
            ]);
            for (child_name, child) in &children {
                assert!(
                    bounds_contains(interior, *child),
                    "{name} {child_name} {child:?} exceeds the lesson panel {interior:?}"
                );
            }
            for (index, (left_name, left)) in children.iter().enumerate() {
                for (right_name, right) in &children[index + 1..] {
                    assert!(
                        bounds_are_disjoint(*left, *right),
                        "{name} {left_name} overlaps {right_name}"
                    );
                }
            }
            for line in &lines {
                assert!(
                    bounds_contains(
                        ui_box_bounds(trial_lesson_copy_box()),
                        text_bounds(line.x, line.y, &line.text, 1)
                    ),
                    "{name} line `{}` leaves the copy region",
                    line.text
                );
            }
            assert_eq!(lines.last().unwrap().tone, LessonTone::AnswerWhy);
            assert!(lines[0].text.starts_with(if correct { "+ " } else { "- " }));
        }

        let legacy_panel = ui_box_bounds(QUIZ_LESSON_BOX);
        let legacy_interior = LayoutBounds {
            x: legacy_panel.x + 1,
            y: legacy_panel.y + 1,
            width: legacy_panel.width - 2,
            height: legacy_panel.height - 2,
        };
        let legacy_lines = lesson_lines(&question, 1, false, quiz_lesson_copy_box());
        for (index, line) in legacy_lines.iter().enumerate() {
            let bounds = text_bounds(line.x, line.y, &line.text, 1);
            assert!(
                bounds_contains(legacy_interior, bounds),
                "legacy {}",
                line.text
            );
            for other in &legacy_lines[index + 1..] {
                assert!(bounds_are_disjoint(
                    bounds,
                    text_bounds(other.x, other.y, &other.text, 1)
                ));
            }
        }
        let legacy_banner = text_bounds(5, 151, "RUNE III AWAKENS", 1);
        let legacy_prompt = text_bounds(
            235 - text_width(LESSON_CONTINUE, 1),
            151,
            LESSON_CONTINUE,
            1,
        );
        assert!(bounds_are_disjoint(legacy_banner, legacy_prompt));
        assert!(bounds_are_disjoint(legacy_panel, legacy_banner));
        assert!(bounds_contains(screen, legacy_prompt));
        let legacy_leave = text_bounds(
            235 - text_width("B AGAIN:LEAVE", 1),
            151,
            "B AGAIN:LEAVE",
            1,
        );
        assert!(bounds_are_disjoint(
            text_bounds(5, 151, "A:ANSWER", 1),
            legacy_leave
        ));
        assert!(bounds_contains(screen, legacy_leave));

        for tone in [
            LessonTone::Misconception,
            LessonTone::MisconceptionWhy,
            LessonTone::Answer,
            LessonTone::AnswerWhy,
        ] {
            let ratio = contrast_ratio(tone.color(), VOID);
            assert!(
                ratio >= 4.5,
                "{tone:?} contrast {ratio:.2}:1 on the lesson panel"
            );
        }
        for (name, color) in [("lens", CYAN_DIM), ("continue", CYAN)] {
            let ratio = contrast_ratio(color, VOID);
            assert!(
                ratio >= 4.5,
                "{name} contrast {ratio:.2}:1 on the lesson panel"
            );
        }
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
    }

    #[test]
    fn oracle_result_waits_for_a_safe_screen_boundary() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        engine.app.world_mut().resource_mut::<GameState>().screen = Screen::Quiz;
        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                questions: vec![QuizQuestion {
                    question: "NEW BATCH".into(),
                    choices: vec!["A".into()],
                    answer: 0,
                    ..Default::default()
                }],
            },
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
    fn opening_scenes_render_distinct_frames() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        let boot = engine.frame().to_vec();

        issue(&mut engine, EngineCommand::BootComplete);
        let copyright = engine.frame().to_vec();
        assert_ne!(copyright, boot);

        for _ in 0..179 {
            engine.update();
        }
        let fanfare_impact = engine.frame().to_vec();
        assert_ne!(fanfare_impact, copyright);

        for _ in 0..120 {
            engine.update();
        }
        let fanfare_oracle = engine.frame().to_vec();
        assert_ne!(fanfare_oracle, fanfare_impact);

        for _ in 0..210 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Title);
        assert_ne!(engine.frame(), fanfare_oracle);
    }

    #[test]
    fn oracle_opening_earns_its_brightest_frame_from_a_dormant_start() {
        let mut state = GameState::default();
        let cartridge = oracle_template_cartridge();
        state.machine = Some(SceneMachine::new((*cartridge.machine).clone()));
        state.cartridge = Some(cartridge);
        state.transition(Screen::Copyright);
        let mut frame = Framebuffer::default();

        render_copyright(&mut frame, &state);
        let dormant_bright = color_pixels_in_region(&frame.pixels, CYAN, 0..WIDTH, 0..HEIGHT)
            + color_pixels_in_region(&frame.pixels, AMBER, 0..WIDTH, 0..HEIGHT);
        assert_eq!(
            dormant_bright, 0,
            "the archive begins without emissive light"
        );

        let beats = [
            OpeningBeat::SourceEmber,
            OpeningBeat::ArchiveAnswer,
            OpeningBeat::MemoryVault,
            OpeningBeat::Convergence,
            OpeningBeat::OracleAwakening,
        ];
        let mut frames = Vec::new();
        let mut luminance = Vec::new();
        for beat in beats {
            render_oracle_opening_beat(&mut frame, beat, 66);
            luminance.push(total_luminance(&frame.pixels));
            frames.push(frame.pixels.clone());
        }

        assert!(
            luminance.windows(2).all(|pair| pair[0] < pair[1]),
            "each authored story scene must increase luminance toward the Oracle climax: {luminance:?}"
        );
        assert_eq!(
            frames.iter().collect::<HashSet<_>>().len(),
            5,
            "the opening must render five distinct authored scenes"
        );
    }

    #[test]
    fn oracle_templates_produce_nine_distinct_native_scene_frames() {
        let mut cartridge = oracle_template_cartridge();
        cartridge.questions[0] = QuizQuestion {
            question: "WHY SEPARATE GAME STATE FROM THE DEVICE SHELL?".into(),
            choices: vec![
                "TO KEEP RESPONSIBILITIES CLEAR".into(),
                "TO DUPLICATE RUNTIME STATE".into(),
                "TO HIDE INPUT TRANSITIONS".into(),
                "TO COUPLE RENDERING TO CSS".into(),
            ],
            answer: 0,
            ..Default::default()
        };
        let machine = SceneMachine::new((*cartridge.machine).clone());
        let mut state = GameState {
            cartridge: Some(cartridge),
            machine: Some(machine),
            quiz: Some(QuizRun {
                completed_batches: 3,
                hearts: 2,
                score: 420,
                level: 4,
                streak: 3,
                leveled_up: true,
                ..QuizRun::new()
            }),
            questions_loading: true,
            screen_ticks: 90,
            oracle_drops: vec![
                OracleDrop {
                    x: 82,
                    y: 58,
                    kind: OracleDropKind::Data,
                },
                OracleDrop {
                    x: 158,
                    y: 78,
                    kind: OracleDropKind::Bug,
                },
            ],
            ..Default::default()
        };

        let mut previews = Vec::new();
        state.screen_ticks = 60;
        let mut boot = Framebuffer::default();
        render_boot(&mut boot, &state);
        maybe_write_preview("00-boot", &boot.pixels);
        for _ in 0..180 {
            state.screen_ticks = state.screen_ticks.saturating_add(1);
            state.tick_machine();
        }
        for (name, advance_ticks) in [
            ("02a-source-ember", 90),
            ("02b-archive-answer", 54),
            ("02c-memory-vault", 66),
            ("02d-convergence", 66),
            ("02e-oracle-awakening", 78),
        ] {
            for _ in 0..advance_ticks {
                state.screen_ticks = state.screen_ticks.saturating_add(1);
                state.tick_machine();
            }
            let mut frame = Framebuffer::default();
            render_oracle_awakening(&mut frame, &state);
            maybe_write_preview(name, &frame.pixels);
            previews.push(frame.pixels);
        }
        for (name, renderer) in [
            (
                "01-chronicle",
                render_oracle_chronicle as fn(&mut Framebuffer, &GameState),
            ),
            ("03-title", render_oracle_title),
            ("04-menu", render_oracle_menu),
            ("05-atelier", render_oracle_atelier),
            ("06-sanctum", render_oracle_sanctum),
            ("07-trial", render_oracle_trial),
            ("08-ascension", render_oracle_ascension),
            ("09-aftermath", render_oracle_aftermath),
        ] {
            state.screen_ticks = match name {
                "03-title" => 60,
                _ => 90,
            };
            let mut frame = Framebuffer::default();
            renderer(&mut frame, &state);
            maybe_write_preview(name, &frame.pixels);
            previews.push(frame.pixels);
        }

        let cartridge = state.cartridge.as_mut().unwrap();
        cartridge.lessons = journal_lessons();
        cartridge.mastery = journal_mastery();
        for (name, page) in [("oracle-codex-mastery", 0), ("oracle-codex-lesson", 2)] {
            state.codex_page = page;
            let mut frame = Framebuffer::default();
            render_oracle_codex(&mut frame, &state);
            maybe_write_preview(name, &frame.pixels);
            previews.push(frame.pixels);
        }
        state.codex_page = 0;

        state.quiz.as_mut().unwrap().selected = 1;
        state.quiz.as_mut().unwrap().feedback = Some((false, QUIZ_FEEDBACK_TICKS));
        let mut review = Framebuffer::default();
        render_oracle_trial(&mut review, &state);
        maybe_write_preview("07b-trial-review", &review.pixels);

        state.quiz.as_mut().unwrap().feedback = None;
        for (name, score, streak) in [
            ("07c-trial-rune-two", 900, 6),
            ("07d-trial-rune-three", 1_800, 9),
        ] {
            state.quiz.as_mut().unwrap().score = score;
            state.quiz.as_mut().unwrap().streak = streak;
            let mut threshold = Framebuffer::default();
            render_oracle_trial(&mut threshold, &state);
            maybe_write_preview(name, &threshold.pixels);
        }

        state.oracle_data = 9;
        state.oracle_bug_hits = 5;
        let mut datafall_thresholds = Framebuffer::default();
        render_oracle_sanctum(&mut datafall_thresholds, &state);
        maybe_write_preview("06b-sanctum-thresholds", &datafall_thresholds.pixels);

        let distinct = previews.iter().collect::<HashSet<_>>();
        assert_eq!(
            distinct.len(),
            15,
            "every reachable scene needs its own authored composition"
        );
        assert!(previews.iter().all(|frame| frame.len() == FRAME_BYTES));
    }

    #[test]
    fn quiz_copy_reserves_visible_letter_spacing_inside_its_panels() {
        assert_eq!(GLYPH_ADVANCE, GLYPH_WIDTH + 1);
        assert!(text_width(&"Q".repeat(QUIZ_QUESTION_COLUMNS), 1) <= 186);
        assert!(text_width(&"C".repeat(QUIZ_CHOICE_CHARS), 1) <= 196);
        assert_eq!(
            wrap_text(
                "WHY SEPARATE GAME STATE FROM THE DEVICE SHELL?",
                QUIZ_QUESTION_COLUMNS,
            ),
            vec!["WHY SEPARATE GAME STATE FROM", "THE DEVICE SHELL?"]
        );
    }

    #[test]
    fn oracle_drop_sprites_are_authored_distinct_and_contained() {
        assert_ne!(ORACLE_DATA_DROP, ORACLE_BUG_DROP);

        for (name, sprite) in [
            ("data", ORACLE_DATA_DROP.as_slice()),
            ("bug", ORACLE_BUG_DROP.as_slice()),
        ] {
            let visible_pixels = sprite
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[3] > 0)
                .count();
            let bright_pixels = sprite
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| {
                    pixel[3] > 0 && pixel[..3].iter().copied().max().unwrap_or(0) >= 128
                })
                .count();
            assert!(
                visible_pixels >= 45,
                "the {name} sprite needs a readable authored silhouette"
            );
            assert!(
                bright_pixels >= 20,
                "the {name} sprite needs a high-contrast luminous core"
            );
        }

        let playfield = LayoutBounds {
            x: 0,
            y: 15,
            width: WIDTH as i32,
            height: 128,
        };
        let offset = DROP_SPRITE_SIZE as i32 / 2;
        for x in [24, 54, 82, 112, 142, 210] {
            for y in [30, 127] {
                let drop = LayoutBounds {
                    x: x - offset,
                    y: y - offset,
                    width: DROP_SPRITE_SIZE as i32,
                    height: DROP_SPRITE_SIZE as i32,
                };
                assert!(
                    bounds_contains(playfield, drop),
                    "drop at ({x}, {y}) exceeds the sanctum playfield"
                );
            }
        }
    }

    #[test]
    fn oracle_progression_changes_the_sanctum_without_relying_on_level_text() {
        let mut state = GameState {
            cartridge: Some(oracle_template_cartridge()),
            quiz: Some(QuizRun::new()),
            questions_loading: true,
            screen_ticks: 90,
            ..Default::default()
        };

        let mut initiate = Framebuffer::default();
        render_oracle_sanctum(&mut initiate, &state);
        state.quiz.as_mut().unwrap().level = 2;
        let mut adept = Framebuffer::default();
        render_oracle_sanctum(&mut adept, &state);
        state.quiz.as_mut().unwrap().level = 4;
        let mut oracle_bound = Framebuffer::default();
        render_oracle_sanctum(&mut oracle_bound, &state);

        assert_ne!(initiate.pixels, adept.pixels);
        assert_ne!(adept.pixels, oracle_bound.pixels);
        assert_eq!(
            color_pixels_in_region(&adept.pixels, MAGENTA, 0..WIDTH, 14..100),
            0,
            "Adept should not borrow the final tier's magenta crest"
        );
        assert!(
            color_pixels_in_region(&oracle_bound.pixels, MAGENTA, 0..WIDTH, 14..100) > 0,
            "Oracle-bound should add a final non-numeric visual channel"
        );
    }

    #[test]
    fn late_fanfare_keeps_its_own_frame_until_title_transition() {
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
        for _ in 0..240 {
            engine.update();
        }

        assert_eq!(engine.screen(), Screen::OpeningFanfare);
        assert_eq!(&engine.frame()[..4], &[INK.0, INK.1, INK.2, 255]);
    }

    #[test]
    fn copyright_frame_reflects_cartridge_provenance() {
        let mut first_cartridge = quiz_cartridge();
        first_cartridge.provenance.authors = vec!["ADA LOVELACE".into()];
        first_cartridge.provenance.first_year = Some(1842);
        first_cartridge.provenance.latest_year = Some(1843);
        let mut first = GameEngine::new();
        issue(&mut first, EngineCommand::Cartridge(Some(first_cartridge)));
        issue(&mut first, EngineCommand::Power(true));
        issue(&mut first, EngineCommand::BootComplete);

        let mut second_cartridge = quiz_cartridge();
        second_cartridge.provenance.authors = vec!["GRACE HOPPER".into()];
        second_cartridge.provenance.first_year = Some(1944);
        second_cartridge.provenance.latest_year = Some(1992);
        let mut second = GameEngine::new();
        issue(
            &mut second,
            EngineCommand::Cartridge(Some(second_cartridge)),
        );
        issue(&mut second, EngineCommand::Power(true));
        issue(&mut second, EngineCommand::BootComplete);

        assert_ne!(first.frame(), second.frame());
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

    const WORST_LESSON_QUESTION: &str =
        "WHICH GUARANTEE KEEPS THE ENGINE AND THE DEVICE SHELL FROM EVER DISAGREEING ABOUT THE ACTIVE SCENE?";
    const WORST_LESSON_RATIONALE: &str =
        "THE SHELL FORWARDS INPUT EDGES AND DRAWS FRAMES, SO ONLY THE ENGINE CAN MOVE THE SCENE MACHINE.";

    fn journal_lessons() -> Vec<Lesson> {
        vec![
            Lesson {
                question: "WHO OWNS THE GAME LOOP?".into(),
                answer: "THE HEADLESS BEVY ENGINE".into(),
                rationale: "THE SHELL ONLY DRAWS FRAMES AND FORWARDS BUTTON EDGES.".into(),
                concept: Some(Concept::Responsibility),
                outstanding: false,
            },
            Lesson {
                question: WORST_LESSON_QUESTION.into(),
                answer: "ONLY THE ENGINE CHANGES SCENES".into(),
                rationale: WORST_LESSON_RATIONALE.into(),
                concept: Some(Concept::Invariant),
                outstanding: true,
            },
            Lesson {
                question: "WHAT DID THIS OLDER SAVE ASK?".into(),
                answer: "A QUESTION WITHOUT A LENS".into(),
                rationale: String::new(),
                concept: None,
                outstanding: false,
            },
        ]
    }

    fn journal_mastery() -> Mastery {
        Mastery::from([
            (
                Concept::Purpose,
                LensRecord {
                    first_try: 1,
                    ..LensRecord::default()
                },
            ),
            (
                Concept::Responsibility,
                LensRecord {
                    first_try: 2,
                    redeemed: 1,
                    missed: 1,
                },
            ),
            (
                Concept::Invariant,
                LensRecord {
                    first_try: 4,
                    redeemed: 1,
                    missed: 3,
                },
            ),
            (
                Concept::Tradeoff,
                LensRecord {
                    missed: 4,
                    ..LensRecord::default()
                },
            ),
        ])
    }

    fn journal_cartridge() -> CartridgeSpec {
        let mut cartridge = quiz_cartridge();
        cartridge.lessons = journal_lessons();
        cartridge.mastery = journal_mastery();
        cartridge
    }

    fn quiz_menu_engine(cartridge: CartridgeSpec) -> GameEngine {
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        press(&mut engine, Button::Start);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        engine
    }

    fn game_state(engine: &GameEngine) -> &GameState {
        engine.app.world().resource::<GameState>()
    }

    #[test]
    fn quiz_menu_opens_the_codex_when_the_scene_graph_routes_to_it() {
        let mut engine = quiz_menu_engine(journal_cartridge());
        assert_eq!(
            quiz_menu_second_option(game_state(&engine)),
            "OPEN THE CODEX"
        );

        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        assert_eq!(game_state(&engine).codex_page, 0);

        press(&mut engine, Button::Right);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        assert_eq!(
            game_state(&engine).menu_selected,
            1,
            "returning from the Codex keeps its menu option focused"
        );

        press(&mut engine, Button::Start);
        assert_eq!(engine.screen(), Screen::Codex);
        assert_eq!(
            game_state(&engine).codex_page,
            0,
            "every visit opens on the mastery overview"
        );

        press(&mut engine, Button::B);
        press(&mut engine, Button::B);
        assert_eq!(
            engine.screen(),
            Screen::Title,
            "B on the menu still returns to the title"
        );
    }

    #[test]
    fn dogfood_manifest_routes_its_menu_into_the_templated_codex() {
        let mut cartridge = oracle_template_cartridge();
        cartridge.lessons = journal_lessons();
        cartridge.mastery = journal_mastery();
        let mut engine = quiz_menu_engine(cartridge);
        maybe_write_preview("oracle-menu-journal", engine.frame());
        press(&mut engine, Button::Down);
        maybe_write_preview("oracle-menu-codex-focus", engine.frame());

        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        let state = game_state(&engine);
        assert!(state.uses_visual_template(VisualTemplate::Codex));
        let mut expected = Framebuffer::default();
        render_oracle_codex(&mut expected, state);
        assert_eq!(engine.frame(), expected.pixels.as_slice());

        press(&mut engine, Button::Right);
        press(&mut engine, Button::Right);
        let state = game_state(&engine);
        assert_eq!(state.codex_lesson().map(|(index, _)| index), Some(1));
        let mut expected = Framebuffer::default();
        render_oracle_codex(&mut expected, state);
        assert_eq!(engine.frame(), expected.pixels.as_slice());
    }

    #[test]
    fn quiz_menu_without_a_codex_route_keeps_return_to_title() {
        let route = |signal, target: &str| SceneTransition {
            signal,
            target: target.into(),
            after_ticks: None,
        };
        let machine = SceneMachineDefinition::compile(
            "title",
            vec![
                SceneSpec {
                    id: "title".into(),
                    handler: SceneHandler::Title,
                    transitions: vec![route(SceneSignal::Continue, "quiz-menu")],
                },
                SceneSpec {
                    id: "quiz-menu".into(),
                    handler: SceneHandler::QuizMenu,
                    transitions: vec![
                        route(SceneSignal::NewRun, "character-creation"),
                        route(SceneSignal::Back, "title"),
                    ],
                },
                SceneSpec {
                    id: "character-creation".into(),
                    handler: SceneHandler::CharacterCreation,
                    transitions: vec![route(SceneSignal::Back, "quiz-menu")],
                },
            ],
        )
        .unwrap();
        let mut cartridge = journal_cartridge();
        cartridge.machine = Box::new(machine);
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        press(&mut engine, Button::Start);
        assert_eq!(engine.screen(), Screen::QuizMenu);

        let state = game_state(&engine);
        assert_eq!(quiz_menu_second_option(state), "RETURN TO TITLE");
        assert_eq!(
            state.journal_summary(),
            None,
            "a menu without the Codex keeps its original subtitle"
        );
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Title);
    }

    #[test]
    fn codex_pages_wrap_through_the_journal_without_changing_it() {
        let mut engine = quiz_menu_engine(journal_cartridge());
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        assert_eq!(game_state(&engine).codex_page_count(), 4);
        let _ = engine.take_effects();

        for (button, expected_page) in [
            (Button::Right, 1),
            (Button::Right, 2),
            (Button::Down, 3),
            (Button::R, 0),
            (Button::Left, 3),
            (Button::Up, 2),
            (Button::L, 1),
            (Button::A, 1),
            (Button::Start, 1),
            (Button::Select, 1),
        ] {
            press(&mut engine, button);
            assert_eq!(engine.screen(), Screen::Codex, "{button:?}");
            assert_eq!(game_state(&engine).codex_page, expected_page, "{button:?}");
        }

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        for _ in 0..30 {
            engine.update();
        }
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_eq!(
            game_state(&engine).codex_page,
            2,
            "a held direction turns exactly one page"
        );

        assert!(
            engine.take_effects().is_empty(),
            "reading the Codex requests and records nothing"
        );
        let cartridge = game_state(&engine).cartridge.as_ref().unwrap();
        assert_eq!(cartridge.lessons, journal_lessons());
        assert_eq!(cartridge.mastery, journal_mastery());

        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
    }

    #[test]
    fn empty_codex_says_no_lessons_yet_and_cannot_page() {
        let mut engine = quiz_menu_engine(quiz_cartridge());
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        for button in [Button::Right, Button::Left, Button::R, Button::Down] {
            press(&mut engine, button);
            assert_eq!(game_state(&engine).codex_page, 0);
        }
        assert!(
            color_pixels_in_region(engine.frame(), GOLD, 0..WIDTH, 116..123) > 0,
            "the legacy Codex names its empty journal"
        );
        maybe_write_preview("codex-legacy-empty", engine.frame());

        let state = GameState {
            cartridge: Some(oracle_template_cartridge()),
            ..Default::default()
        };
        let mut empty = Framebuffer::default();
        render_oracle_codex(&mut empty, &state);
        maybe_write_preview("oracle-codex-empty", &empty.pixels);
        let mut expected = Framebuffer::default();
        expected.blit_rgb(ORACLE_CHRONICLE);
        draw_codex_panel(&mut expected, CODEX_TOTALS_BOX);
        expected.centered_text_box(CODEX_TOTALS_BOX, "NO LESSONS YET", AMBER, 1);
        draw_codex_panel(&mut expected, CODEX_PROMPT_BOX);
        expected.centered_text_box(CODEX_PROMPT_BOX, "ANSWER A TRIAL  B:BACK", MIST, 1);
        for bounds in [CODEX_TOTALS_BOX, CODEX_PROMPT_BOX] {
            let x = bounds.x as usize..(bounds.x + bounds.width) as usize;
            let y = bounds.y as usize..(bounds.y + bounds.height) as usize;
            assert_eq!(
                frame_region(&empty.pixels, x.clone(), y.clone()),
                frame_region(&expected.pixels, x, y),
                "the empty journal shows an honest state and how to fill it"
            );
        }
    }

    #[test]
    fn quiz_menu_summarizes_the_journal_when_the_codex_is_routable() {
        let mut engine = quiz_menu_engine(journal_cartridge());
        assert_eq!(
            game_state(&engine).journal_summary().as_deref(),
            Some("LESSONS 03  REVIEW 01")
        );

        let mut state = engine.app.world_mut().resource_mut::<GameState>();
        for lesson in &mut state.cartridge.as_mut().unwrap().lessons {
            lesson.outstanding = false;
        }
        assert_eq!(
            state.journal_summary().as_deref(),
            Some("LESSONS 03  ALL CLEAR")
        );
        state.cartridge.as_mut().unwrap().lessons.clear();
        assert_eq!(state.journal_summary(), None);
    }

    #[test]
    fn codex_mastery_runes_wake_at_the_exact_evidence_thresholds_on_the_rendered_frame() {
        let invariant_row = Concept::ALL
            .iter()
            .position(|concept| *concept == Concept::Invariant)
            .unwrap() as i32;
        for (name, renderer, runes_x, row_y) in [
            (
                "oracle",
                render_oracle_codex_mastery as fn(&mut Framebuffer, &GameState),
                CODEX_LENS_RUNES_X,
                CODEX_LENS_ROW_Y + invariant_row * CODEX_LENS_ROW_PITCH,
            ),
            ("legacy", render_codex_mastery, 128, 32 + invariant_row * 14),
        ] {
            let mut meters = Vec::new();
            for evidence in 0..=6u32 {
                let mut cartridge = quiz_cartridge();
                // Evidence mixes first-try successes and redemptions; misses
                // never light a rune.
                cartridge.mastery.insert(
                    Concept::Invariant,
                    LensRecord {
                        first_try: evidence - evidence / 2,
                        redeemed: evidence / 2,
                        missed: 9,
                    },
                );
                let state = GameState {
                    cartridge: Some(cartridge),
                    ..Default::default()
                };
                let mut frame = Framebuffer::default();
                renderer(&mut frame, &state);
                let x = runes_x as usize..(runes_x + 19) as usize;
                let y = row_y as usize..(row_y + 7) as usize;
                let lit = color_pixels_in_region(&frame.pixels, PARCH, x.clone(), y.clone()) / 5;
                assert_eq!(
                    lit,
                    [0, 1, 1, 2, 2, 3, 3][evidence as usize],
                    "{name}: {evidence} evidence must light the declared rune count"
                );
                let stage_color = [None, Some(CYAN), Some(AMBER), Some(MAGENTA)][lit];
                if let Some(color) = stage_color {
                    assert!(
                        color_pixels_in_region(&frame.pixels, color, x.clone(), y.clone()) > 0,
                        "{name}: stage {lit} uses its own rune color"
                    );
                }
                meters.push(frame_region(&frame.pixels, x, y));
            }
            for (below, at) in [(0, 1), (2, 3), (4, 5)] {
                assert_ne!(
                    meters[below], meters[at],
                    "{name}: crossing {MASTERY_THRESHOLDS:?} changes the meter"
                );
            }
            for (at, above) in [(1, 2), (3, 4), (5, 6)] {
                assert_eq!(
                    meters[at], meters[above],
                    "{name}: the meter holds between thresholds"
                );
            }
        }
    }

    #[test]
    fn codex_lesson_pages_show_the_answer_and_mark_outstanding_reviews() {
        let mut cartridge = oracle_template_cartridge();
        cartridge.lessons = journal_lessons();
        cartridge.mastery = journal_mastery();
        let mut state = GameState {
            cartridge: Some(cartridge),
            ..Default::default()
        };
        let status = (44..127, 20..27);
        let answer = (
            CODEX_ANSWER_BOX.x as usize..(CODEX_ANSWER_BOX.x + CODEX_ANSWER_BOX.width) as usize,
            CODEX_ANSWER_BOX.y as usize..(CODEX_ANSWER_BOX.y + CODEX_ANSWER_BOX.height) as usize,
        );
        let rationale = (23..226, 116..139);
        let render = |state: &GameState, page, legacy: bool| {
            let mut state_frame = Framebuffer::default();
            let mut paged = GameState {
                cartridge: state.cartridge.clone(),
                codex_page: page,
                ..Default::default()
            };
            paged.hero_style = state.hero_style;
            if legacy {
                render_codex(&mut state_frame, &paged);
            } else {
                render_oracle_codex(&mut state_frame, &paged);
            }
            state_frame.pixels
        };

        let learned = render(&state, 1, false);
        maybe_write_preview("oracle-codex-lesson-learned", &learned);
        assert!(color_pixels_in_region(&learned, CYAN, status.0.clone(), status.1.clone()) > 0);
        assert_eq!(
            color_pixels_in_region(&learned, AMBER, status.0.clone(), status.1.clone()),
            0
        );
        assert!(color_pixels_in_region(&learned, GREEN, answer.0.clone(), answer.1.clone()) > 0);

        let pending = render(&state, 2, false);
        assert!(
            color_pixels_in_region(&pending, AMBER, status.0.clone(), status.1.clone()) > 0,
            "an outstanding lesson reads REVIEW PENDING in amber"
        );
        assert!(
            color_pixels_in_region(&pending, PARCH, rationale.0.clone(), rationale.1.clone()) > 0
        );

        let legacy_lesson = render(&state, 3, false);
        maybe_write_preview("oracle-codex-lesson-no-rationale", &legacy_lesson);
        assert!(
            color_pixels_in_region(
                &legacy_lesson,
                MIST,
                rationale.0.clone(),
                rationale.1.clone()
            ) > 0
        );
        assert_eq!(
            color_pixels_in_region(&legacy_lesson, PARCH, rationale.0.clone(), rationale.1),
            0,
            "a lesson without a rationale says so instead of inventing one"
        );

        state.cartridge.as_mut().unwrap().codequest = None;
        let plain = render(&state, 2, true);
        maybe_write_preview("codex-legacy-lesson", &plain);
        assert!(color_pixels_in_region(&plain, AMBER, 5..90, 21..28) > 0);
        assert!(color_pixels_in_region(&plain, GREEN, 21..212, 76..83) > 0);
        let mut plain_state = GameState {
            cartridge: state.cartridge.clone(),
            ..Default::default()
        };
        plain_state.codex_page = 0;
        let mut plain_mastery = Framebuffer::default();
        render_codex(&mut plain_mastery, &plain_state);
        maybe_write_preview("codex-legacy-mastery", &plain_mastery.pixels);
        assert_ne!(plain, plain_mastery.pixels);
    }

    fn brightest_plate_color(plate: &[u8; NATIVE_RGB_BYTES], bounds: LayoutBounds) -> Color {
        let mut brightest = Color::rgb(0, 0, 0);
        for y in bounds.y..bounds.y + bounds.height {
            for x in bounds.x..bounds.x + bounds.width {
                let offset = (y as usize * WIDTH + x as usize) * 3;
                let color = Color::rgb(plate[offset], plate[offset + 1], plate[offset + 2]);
                if relative_luminance(color) > relative_luminance(brightest) {
                    brightest = color;
                }
            }
        }
        brightest
    }

    fn inset(bounds: UiBox) -> LayoutBounds {
        LayoutBounds {
            x: bounds.x + 1,
            y: bounds.y + 1,
            width: bounds.width - 2,
            height: bounds.height - 2,
        }
    }

    #[test]
    fn codex_ui_stays_contained_disjoint_and_readable() {
        let screen = LayoutBounds {
            x: 0,
            y: 0,
            width: WIDTH as i32,
            height: HEIGHT as i32,
        };
        // Measured usable interior of the chronicle plate's archive frame.
        let archive = LayoutBounds {
            x: 62,
            y: 37,
            width: 116,
            height: 77,
        };
        let totals = inset(CODEX_TOTALS_BOX);
        let prompt = inset(CODEX_PROMPT_BOX);
        let header = inset(CODEX_LESSON_HEADER_BOX);
        let question = inset(CODEX_QUESTION_BOX);
        let answer = inset(CODEX_ANSWER_BOX);
        let rationale = inset(CODEX_RATIONALE_BOX);
        let portrait = LayoutBounds {
            x: 8,
            y: 5,
            width: HERO_PORTRAIT_SIZE as i32,
            height: HERO_PORTRAIT_SIZE as i32,
        };

        let heading = centered_text_box_bounds(CODEX_HEADING_BOX, "ORACLE CODEX", 1);
        let widest_lens = Concept::ALL
            .iter()
            .map(|concept| concept.label())
            .max_by_key(|label| label.len())
            .unwrap();
        let mut archive_children = vec![("mastery heading", heading, PARCH)];
        for row in 0..Concept::ALL.len() as i32 {
            let y = CODEX_LENS_ROW_Y + row * CODEX_LENS_ROW_PITCH;
            archive_children.push((
                "lens label",
                text_bounds(CODEX_LENS_LABEL_X, y, widest_lens, 1),
                PARCH,
            ));
            archive_children.push((
                "lens runes",
                LayoutBounds {
                    x: CODEX_LENS_RUNES_X,
                    y,
                    width: 19,
                    height: 7,
                },
                CYAN,
            ));
            archive_children.push((
                "lens pending count",
                text_bounds(CODEX_LENS_PENDING_X, y, "!99", 1),
                AMBER,
            ));
        }
        let worst_totals = "LEARNED 99  REVIEW 99";
        let lesson_counter = text_bounds(44, 7, &codex_lesson_counter(998, 999), 1);
        let lesson_lens = text_bounds(214 - 6 - text_width(widest_lens, 1), 7, widest_lens, 1);
        let lesson_runes = LayoutBounds {
            x: 214,
            y: 7,
            width: 19,
            height: 7,
        };
        let general_lens = text_bounds(214 + 19 - text_width("GENERAL", 1), 7, "GENERAL", 1);
        let lesson_status = text_bounds(44, 20, "REVIEW PENDING", 1);
        let lesson_controls = text_bounds(147, 20, "L/R:PAGE B:BACK", 1);
        let question_block = LayoutBounds {
            x: CODEX_TEXT_X,
            y: CODEX_QUESTION_Y,
            width: text_width(&"Q".repeat(QUIZ_QUESTION_COLUMNS), 1),
            height: QUIZ_QUESTION_ROWS as i32 * LINE_HEIGHT - 1,
        };
        let answer_copy = text_bounds(
            CODEX_ANSWER_TEXT_X,
            CODEX_ANSWER_Y,
            &"A".repeat(QUIZ_CHOICE_CHARS),
            1,
        );
        let answer_rune = LayoutBounds {
            x: CODEX_TEXT_X,
            y: CODEX_ANSWER_Y,
            width: 5,
            height: 7,
        };
        let rationale_heading = text_bounds(CODEX_TEXT_X, CODEX_WHY_Y, "WHY IT HOLDS", 1);
        let rationale_block = LayoutBounds {
            x: CODEX_TEXT_X,
            y: CODEX_RATIONALE_Y,
            width: text_width(&"R".repeat(RATIONALE_COLUMNS), 1),
            height: RATIONALE_ROWS as i32 * LINE_HEIGHT - 1,
        };
        let missing_rationale = text_bounds(
            CODEX_TEXT_X,
            CODEX_RATIONALE_Y,
            "NO RATIONALE WAS RECORDED",
            1,
        );
        let menu_option =
            centered_text_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1], "OPEN THE CODEX", 1);
        let menu_subtitle =
            centered_text_box_bounds(GATEWAY_MENU_SUBTITLE_BOX, "LESSONS 99  REVIEW 99", 1);

        let mut contained = archive_children
            .iter()
            .map(|(name, child, _)| (*name, archive, *child))
            .collect::<Vec<_>>();
        contained.extend([
            (
                "totals",
                totals,
                centered_text_box_bounds(CODEX_TOTALS_BOX, worst_totals, 1),
            ),
            (
                "empty journal",
                totals,
                centered_text_box_bounds(CODEX_TOTALS_BOX, "NO LESSONS YET", 1),
            ),
            (
                "reading controls",
                prompt,
                centered_text_box_bounds(CODEX_PROMPT_BOX, "L/R:READ LESSONS  B:BACK", 1),
            ),
            (
                "empty guidance",
                prompt,
                centered_text_box_bounds(CODEX_PROMPT_BOX, "ANSWER A TRIAL  B:BACK", 1),
            ),
            ("lesson counter", header, lesson_counter),
            ("lesson lens", header, lesson_lens),
            ("lesson lens runes", header, lesson_runes),
            ("lesson without lens", header, general_lens),
            ("lesson status", header, lesson_status),
            ("lesson controls", header, lesson_controls),
            ("lesson question", question, question_block),
            ("lesson answer", answer, answer_copy),
            ("lesson answer rune", answer, answer_rune),
            ("rationale heading", rationale, rationale_heading),
            ("rationale", rationale, rationale_block),
            ("missing rationale", rationale, missing_rationale),
            (
                "menu codex option",
                ui_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1]),
                menu_option,
            ),
            (
                "menu journal summary",
                ui_box_bounds(GATEWAY_MENU_SUBTITLE_BOX),
                menu_subtitle,
            ),
        ]);
        for (name, container, child) in contained {
            assert!(
                bounds_contains(container, child),
                "{name} {child:?} exceeds its container {container:?}"
            );
            assert!(
                bounds_contains(screen, child),
                "{name} {child:?} exceeds the native frame"
            );
        }
        assert!(horizontal_centers_align(
            ui_box_bounds(CODEX_HEADING_BOX),
            heading
        ));

        for (index, (left_name, left, _)) in archive_children.iter().enumerate() {
            for (right_name, right, _) in &archive_children[index + 1..] {
                assert!(
                    bounds_are_disjoint(*left, *right),
                    "{left_name} {left:?} overlaps {right_name} {right:?}"
                );
            }
        }
        for (name, left, right) in [
            (
                "archive and totals",
                archive,
                ui_box_bounds(CODEX_TOTALS_BOX),
            ),
            (
                "totals and prompt",
                ui_box_bounds(CODEX_TOTALS_BOX),
                ui_box_bounds(CODEX_PROMPT_BOX),
            ),
            (
                "portrait and header",
                portrait,
                ui_box_bounds(CODEX_LESSON_HEADER_BOX),
            ),
            ("counter and lens", lesson_counter, lesson_lens),
            ("counter and lensless label", lesson_counter, general_lens),
            ("lens and runes", lesson_lens, lesson_runes),
            ("counter and status", lesson_counter, lesson_status),
            ("status and controls", lesson_status, lesson_controls),
            (
                "header and question",
                ui_box_bounds(CODEX_LESSON_HEADER_BOX),
                ui_box_bounds(CODEX_QUESTION_BOX),
            ),
            (
                "question and answer",
                ui_box_bounds(CODEX_QUESTION_BOX),
                answer,
            ),
            ("answer rune and answer", answer_rune, answer_copy),
            (
                "answer and rationale",
                ui_box_bounds(CODEX_ANSWER_BOX),
                ui_box_bounds(CODEX_RATIONALE_BOX),
            ),
            (
                "rationale heading and copy",
                rationale_heading,
                rationale_block,
            ),
        ] {
            assert!(
                bounds_are_disjoint(left, right),
                "{name} overlap: {left:?} and {right:?}"
            );
        }

        // Copy drawn straight onto a plate must clear every ornament: test it
        // against the brightest plate pixel inside its own glyph cells.
        let mut plate_copy = archive_children
            .iter()
            .map(|(name, bounds, color)| (*name, ORACLE_CHRONICLE, *bounds, *color))
            .collect::<Vec<_>>();
        plate_copy.push(("unlit lens", ORACLE_CHRONICLE, archive_children[1].1, MIST));
        for (name, color) in [("amber rune", AMBER), ("magenta rune", MAGENTA)] {
            plate_copy.push((name, ORACLE_CHRONICLE, archive_children[2].1, color));
        }
        for (name, plate, bounds, color) in plate_copy {
            let fill = brightest_plate_color(plate, bounds);
            let ratio = contrast_ratio(color, fill);
            assert!(
                ratio >= 4.5,
                "{name} contrast {ratio:.2}:1 against plate fill {:?} is below 4.5:1",
                (fill.0, fill.1, fill.2)
            );
        }
        for (name, foreground, background) in [
            ("legacy counter", SKY, INK),
            ("legacy lens", PARCH, INK),
            ("legacy status", AMBER, NAVY),
            ("legacy learned", CYAN, NAVY),
            ("legacy unlit lens", MIST, NAVY),
            ("legacy pending", AMBER, NAVY),
            ("lesson answer panel", GREEN, VOID),
            ("lesson question panel", PARCH, VOID),
            ("rationale heading panel", CYAN_DIM, VOID),
            ("lesson status panel", AMBER, VOID),
            ("lesson controls panel", MIST, VOID),
            ("legacy answer", GREEN, INK),
            ("legacy answer marker", GOLD, INK),
            ("legacy empty journal", GOLD, NAVY),
            ("legacy question", PARCH, INK),
            ("legacy rationale heading", SKY, INK),
        ] {
            let ratio = contrast_ratio(foreground, background);
            assert!(ratio >= 4.5, "{name} contrast {ratio:.2}:1 is below 4.5:1");
        }
    }

    #[test]
    fn codex_legacy_layout_keeps_worst_case_copy_inside_its_panels() {
        let question_box = LayoutBounds {
            x: 6,
            y: 32,
            width: 228,
            height: 36,
        };
        let rationale_box = LayoutBounds {
            x: 6,
            y: 91,
            width: 228,
            height: 50,
        };
        let mastery_box = LayoutBounds {
            x: 31,
            y: 25,
            width: 178,
            height: 80,
        };
        for (name, container, child) in [
            (
                "question",
                question_box,
                LayoutBounds {
                    x: 11,
                    y: 35,
                    width: text_width(&"Q".repeat(QUIZ_QUESTION_COLUMNS), 1),
                    height: QUIZ_QUESTION_ROWS as i32 * LINE_HEIGHT - 1,
                },
            ),
            (
                "rationale heading",
                rationale_box,
                text_bounds(11, 95, "WHY IT HOLDS", 1),
            ),
            (
                "rationale",
                rationale_box,
                LayoutBounds {
                    x: 11,
                    y: 107,
                    width: text_width(&"R".repeat(RATIONALE_COLUMNS), 1),
                    height: RATIONALE_ROWS as i32 * LINE_HEIGHT - 1,
                },
            ),
            (
                "first lens",
                mastery_box,
                text_bounds(42, 32, "INVARIANTS", 1),
            ),
            (
                "last pending count",
                mastery_box,
                text_bounds(156, 32 + 4 * 14, "!99", 1),
            ),
        ] {
            assert!(
                bounds_contains(container, child),
                "{name} {child:?} exceeds {container:?}"
            );
        }
        assert!(bounds_are_disjoint(
            text_bounds(5, 4, &codex_lesson_counter(998, 999), 1),
            text_bounds(214 - 6 - text_width("INVARIANTS", 1), 4, "INVARIANTS", 1),
        ));
        assert!(bounds_are_disjoint(
            centered_text_bounds(116, "LEARNED 99  REVIEW 99", 1),
            text_bounds(5, 151, "L/R:PAGE", 1),
        ));
    }

    #[test]
    fn codex_fixture_copy_is_worst_case_for_the_lesson_panels() {
        assert_eq!(
            wrap_text(WORST_LESSON_QUESTION, QUIZ_QUESTION_COLUMNS).len(),
            QUIZ_QUESTION_ROWS
        );
        let rationale = wrap_text(WORST_LESSON_RATIONALE, RATIONALE_COLUMNS);
        assert_eq!(rationale.len(), RATIONALE_ROWS);
        assert_eq!(rationale[0].chars().count(), RATIONALE_COLUMNS);
        assert!(crate::learning::rationale_fits(WORST_LESSON_RATIONALE));
        for lesson in journal_lessons() {
            assert!(
                lesson.answer.chars().count() <= QUIZ_CHOICE_CHARS,
                "lesson answers are committed quiz choices and share their limit"
            );
        }
    }

    #[test]
    fn wrapping_never_splits_into_oversized_lines() {
        let lines = wrap_text("alpha beta supercalifragilistic", 8);
        assert!(lines.iter().all(|line| line.chars().count() <= 8));
    }

    /// The whole learning loop across the quiz and the Codex: a miss is
    /// journaled as a pending review, returns after the retry gap, is redeemed,
    /// and the Codex rereads the same journal and mastery the quiz wrote.
    #[test]
    fn a_missed_concept_is_journaled_retried_redeemed_and_reread_in_the_codex() {
        let mut engine = batch_quiz_engine(QUESTION_BATCH_SIZE);
        let missed = current_question(&engine);
        commit(&mut engine, false);
        {
            let cartridge = engine_state(&engine).cartridge.as_ref().unwrap();
            let lesson = cartridge
                .lessons
                .iter()
                .find(|lesson| lesson.question == missed.question)
                .expect("committing a miss journals the lesson");
            assert!(lesson.outstanding, "a miss is pending review");
            assert_eq!(lesson.answer, missed.choices[missed.answer]);
            assert_eq!(lesson.rationale, missed.rationales[missed.answer]);
            assert_eq!(cartridge.mastery[&Concept::Responsibility].missed, 1);
        }
        finish_lesson(&mut engine);

        for _ in 0..RETRY_GAP {
            assert!(!current_question(&engine).review);
            commit(&mut engine, true);
            finish_lesson(&mut engine);
        }
        let retry = current_question(&engine);
        assert!(retry.review, "the miss returns after the retry gap");
        assert_eq!(retry.question, missed.question);
        commit(&mut engine, true);
        {
            let state = engine_state(&engine);
            assert!(state.quiz.as_ref().unwrap().redeemed);
            let cartridge = state.cartridge.as_ref().unwrap();
            let lesson = cartridge
                .lessons
                .iter()
                .find(|lesson| lesson.question == missed.question)
                .unwrap();
            assert!(!lesson.outstanding, "redemption clears the pending review");
            let record = cartridge.mastery[&Concept::Responsibility];
            assert_eq!((record.missed, record.redeemed), (1, 1));
        }
        finish_lesson(&mut engine);

        press(&mut engine, Button::B);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        let journal = engine_state(&engine)
            .cartridge
            .as_ref()
            .unwrap()
            .lessons
            .clone();
        if engine_state(&engine).menu_selected == 0 {
            press(&mut engine, Button::Down);
        }
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        for _ in 0..journal.len() {
            press(&mut engine, Button::Right);
            let (_, lesson) = engine_state(&engine)
                .codex_lesson()
                .expect("every journal page shows a lesson");
            assert!(journal.contains(lesson));
        }
        assert_eq!(
            engine_state(&engine).cartridge.as_ref().unwrap().lessons,
            journal,
            "reading the Codex never changes the journal"
        );
    }
}
