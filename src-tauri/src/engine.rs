use std::collections::{HashSet, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::codequest::{CodeQuestConfig, GameType, VisualTemplate};
use crate::external_tools;
use crate::font5x7::{glyph, GLYPH_ADVANCE, GLYPH_WIDTH, LINE_HEIGHT};
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
pub const QUIZ_QUESTION_COLUMNS: usize = 37;
pub const QUIZ_QUESTION_ROWS: usize = 4;
pub const QUIZ_CHOICE_CHARS: usize = 35;
const QUESTION_BATCH_SIZE: usize = 6;
const FRAME_TIME: Duration = Duration::from_nanos(16_666_667);
const ORACLE_HERO_MIN_X: i32 = 8;
const ORACLE_HERO_MAX_X: i32 = 210;
const ORACLE_HERO_SPEED: i32 = 2;
const ORACLE_DROP_INTERVAL: u64 = 30;
const ORACLE_COLLISION_Y: i32 = 100;
const QUIZ_FEEDBACK_TICKS: u16 = 45;
const LEVEL_UP_HOLD_TICKS: u64 = 60;

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
const HERO_ACCESSORIES: [&str; 6] = [
    "MUSTACHE", "FEDORA", "BOW TIE", "MONOCLE", "CROWN", "SHADES",
];
const HERO_WEAPONS: [&str; 6] = [
    "SWORD",
    "ORB STAFF",
    "PIPE",
    "WARHAMMER",
    "BOW",
    "SHELL SHIELD",
];
const HERO_STYLE_COLORS: [Color; 5] = [RED, SKY, GREEN, GOLD, PLUM];

const ORACLE_CHRONICLE: &[u8; NATIVE_RGB_BYTES] = include_bytes!("../assets/oracle/chronicle.rgb");
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
const MENU_FOOTER_BOX: UiBox = UiBox {
    x: 39,
    y: 144,
    width: 162,
    height: 16,
};
const ATELIER_HEADER_BOX: UiBox = UiBox {
    x: 8,
    y: 4,
    width: 126,
    height: 16,
};
const ATELIER_ACCESSORY_BOX: UiBox = UiBox {
    x: 8,
    y: 119,
    width: 100,
    height: 16,
};
const ATELIER_BIND_BOX: UiBox = UiBox {
    x: 137,
    y: 112,
    width: 55,
    height: 21,
};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PresentationTier {
    Initiate,
    Adept,
    OracleBound,
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

#[derive(Clone, Debug)]
pub struct QuizQuestion {
    pub question: String,
    pub choices: Vec<String>,
    pub answer: usize,
}

pub type QuestionLoader =
    Arc<dyn Fn(String, u32, usize) -> Vec<QuizQuestion> + Send + Sync + 'static>;

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
        for (source, destination) in rgb.chunks_exact(3).zip(self.pixels.chunks_exact_mut(4)) {
            destination.copy_from_slice(&[source[0], source[1], source[2], 255]);
        }
    }

    fn blit_rgb_graded(&mut self, rgb: &[u8; NATIVE_RGB_BYTES], base: u16, cyan: u16, gold: u16) {
        for (source, destination) in rgb.chunks_exact(3).zip(self.pixels.chunks_exact_mut(4)) {
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
            .chunks_exact(3)
            .zip(self.pixels.chunks_exact_mut(4))
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

    fn wrapped_compact_text(
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
            self.compact_text(x, y + line_no as i32 * LINE_HEIGHT, &line, color);
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
    selected: usize,
    hearts: u8,
    score: u32,
    level: u32,
    streak: u32,
    leveled_up: bool,
    feedback: Option<(bool, u16)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OracleDropKind {
    Data,
    Bug,
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
    cartridge: Option<CartridgeSpec>,
    machine: Option<SceneMachine>,
    screen: Screen,
    screen_ticks: u64,
    held: HashSet<Button>,
    menu_selected: usize,
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
            cartridge: None,
            machine: None,
            screen: Screen::Off,
            screen_ticks: 0,
            held: HashSet::new(),
            menu_selected: 0,
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
    fn transition(&mut self, screen: Screen) {
        if screen == Screen::Oracle && self.screen != Screen::Oracle {
            self.oracle_hero_x = 104;
            self.oracle_drops.clear();
            self.oracle_spawned = 0;
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
    state.quiz = Some(QuizRun {
        question: 0,
        completed_batches: 0,
        selected: 0,
        hearts: 3,
        score: 0,
        level: 1,
        streak: 0,
        leveled_up: false,
        feedback: None,
    });
    state.signal(SceneSignal::HeroReady);
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
    pub fn spawn(question_loader: QuestionLoader) -> Self {
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
                        handle_effect(effect, &engine_sender, &running_child, &question_loader);
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
                    state.signal(SceneSignal::Back);
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
            let Some(run) = state.quiz.as_mut() else {
                return;
            };
            if run.feedback.is_some() {
                return;
            }
            let choice_count = state
                .cartridge
                .as_ref()
                .and_then(|cart| cart.questions.get(run.question))
                .map_or(1, |question| question.choices.len().max(1));
            match button {
                Button::Up => run.selected = (run.selected + choice_count - 1) % choice_count,
                Button::Down => run.selected = (run.selected + 1) % choice_count,
                Button::B => {
                    state.signal(SceneSignal::Back);
                }
                Button::A => {
                    let answer = state
                        .cartridge
                        .as_ref()
                        .and_then(|cart| cart.questions.get(run.question))
                        .map_or(0, |question| question.answer);
                    let correct = run.selected == answer;
                    if correct {
                        run.score += 100;
                        run.streak += 1;
                    } else {
                        run.hearts = run.hearts.saturating_sub(1);
                        run.streak = 0;
                    }
                    run.feedback = Some((correct, QUIZ_FEEDBACK_TICKS));
                }
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
            let question_count = state.question_count();
            let next_batch_end = state
                .quiz
                .as_ref()
                .and_then(|run| state.batch_ends.get(run.completed_batches).copied());
            let mut next_signal = None;
            if let Some(run) = state.quiz.as_mut() {
                if let Some((correct, ticks)) = run.feedback.as_mut() {
                    let _ = correct;
                    *ticks = ticks.saturating_sub(1);
                    if *ticks == 0 {
                        run.feedback = None;
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
                    }
                }
            }
            if let Some(signal) = next_signal {
                state.signal(signal);
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

fn render_oracle_awakening(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_awakening(state.screen_ticks);
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

fn render_quiz_menu(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Menu) {
        render_oracle_menu(frame, state);
        return;
    }
    frame.clear(NAVY);
    frame.centered_text(20, "REPO QUIZ", GOLD, 2);
    frame.outline(34, 58, 172, 62, SKY);
    for (index, label) in ["BEGIN RUN", "RETURN TO TITLE"].iter().enumerate() {
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
    frame.centered_text_box(GATEWAY_MENU_SUBTITLE_BOX, "THE BOND BEGINS HERE", CYAN, 1);
    for (index, (label, detail)) in [
        ("BEGIN THE TRIAL", "FORGE A HERO"),
        ("RETURN TO TITLE", "CLOSE ARCHIVE"),
    ]
    .iter()
    .enumerate()
    {
        let option_box = GATEWAY_MENU_OPTION_BOXES[index];
        let y = option_box.y;
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
        frame.centered_text_in(
            option_box.x,
            y + 2,
            option_box.width,
            label,
            if focused { PARCH } else { MIST },
            1,
        );
        frame.centered_text_in(
            option_box.x,
            y + 10,
            option_box.width,
            detail,
            if focused { AMBER } else { CYAN_DIM },
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
    frame.text(
        81,
        39,
        &format!("GEAR: {}", HERO_ACCESSORIES[state.hero_name]),
        MIST,
        1,
    );
    frame.text(
        81,
        60,
        &format!("WEAPON: {}", HERO_WEAPONS[state.hero_class]),
        MIST,
        1,
    );

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
    frame.centered_text_box(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER", AMBER, 1);
    let bob = ((state.screen_ticks / 22) % 2) as i32;
    draw_hero(frame, 32, 47 - bob, 2, state);
    frame.rect(
        ATELIER_ACCESSORY_BOX.x,
        ATELIER_ACCESSORY_BOX.y,
        ATELIER_ACCESSORY_BOX.width,
        ATELIER_ACCESSORY_BOX.height,
        VOID,
    );
    frame.outline(
        ATELIER_ACCESSORY_BOX.x,
        ATELIER_ACCESSORY_BOX.y,
        ATELIER_ACCESSORY_BOX.width,
        ATELIER_ACCESSORY_BOX.height,
        CYAN_DIM,
    );
    frame.centered_text_box(
        ATELIER_ACCESSORY_BOX,
        HERO_ACCESSORIES[state.hero_name],
        AMBER,
        1,
    );

    let rows = [
        ("NAME", HERO_NAMES[state.hero_name]),
        ("PATH", HERO_CLASSES[state.hero_class]),
        ("AURA", HERO_STYLES[state.hero_style]),
    ];
    for (index, (label, value)) in rows.iter().enumerate() {
        let y = 17 + index as i32 * 32;
        let focused = state.hero_row == index;
        if focused {
            draw_asset_focus(frame, 116, y, 98, 29);
        }
        frame.text(126, y + 6, label, if focused { AMBER } else { CYAN_DIM }, 1);
        frame.compact_text(
            122,
            y + 16,
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
        ATELIER_BIND_BOX,
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
        "QUESTION READY"
    } else if state.questions_loading {
        "CLAUDE THINKING"
    } else if state.question_retry_ticks > 0 {
        "CLAUDE RETRYING"
    } else {
        "CONTACTING CLAUDE"
    };
    let status_width = status.chars().count() as i32 * GLYPH_ADVANCE - 1;
    frame.text(211 - status_width, 2, status, SKY, 1);
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
    frame.centered_text(143, "L/R MOVE  B:BACK", PARCH, 1);
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
        "CLAUDE:READY"
    } else if state.questions_loading {
        "CLAUDE:SCRYING"
    } else if state.question_retry_ticks > 0 {
        "CLAUDE:CLOUDY"
    } else {
        "CLAUDE:CHANNEL"
    };
    let status_width = status.chars().count() as i32 * GLYPH_ADVANCE - 1;
    frame.text(235 - status_width, 5, status, CYAN, 1);

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
    frame.centered_text(149, "L/R:MOVE  B:LEAVE", PARCH, 1);
    frame.text(
        198,
        149,
        &format!("BUG {:02}", state.oracle_bug_hits.min(99)),
        RED,
        1,
    );
}

fn draw_oracle_data(frame: &mut Framebuffer, x: i32, y: i32) {
    frame.outline(x - 4, y - 4, 9, 9, GREEN);
    frame.rect(x - 1, y - 1, 3, 3, SKY);
}

fn draw_oracle_bug(frame: &mut Framebuffer, x: i32, y: i32) {
    frame.rect(x - 3, y - 3, 7, 7, RED);
    frame.pixel(x - 2, y - 2, INK);
    frame.pixel(x + 2, y - 2, INK);
    frame.line(x - 5, y - 5, x + 5, y + 5, RED);
    frame.line(x + 5, y - 5, x - 5, y + 5, RED);
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
    frame.text(
        84,
        4,
        &format!("HP {}", "*".repeat(run.hearts as usize)),
        RED,
        1,
    );
    frame.text(211, 4, &format!("{:04}", run.score), GOLD, 1);
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
    for (index, choice) in question.choices.iter().take(4).enumerate() {
        let y = 73 + index as i32 * 20;
        let mut color = PARCH;
        if run.selected == index {
            frame.rect(5, y - 3, 230, 14, ROYAL);
            frame.text(9, y, ">", GOLD, 1);
        }
        if let Some((correct, _)) = run.feedback {
            if index == question.answer {
                color = GREEN;
            } else if run.selected == index && !correct {
                color = RED;
            }
        }
        frame.text(21, y, &truncate(choice, QUIZ_CHOICE_CHARS), color, 1);
    }
    if run.feedback.is_some() {
        frame.centered_text(151, "REVIEW ANSWER", MIST, 1);
    } else {
        frame.text(5, 151, "A:ANSWER", MIST, 1);
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
    frame.text(
        151,
        7,
        &format!("HP {}", "*".repeat(run.hearts.min(3) as usize)),
        RED,
        1,
    );
    frame.text(215, 7, &format!("{:04}", run.score.min(9999)), AMBER, 1);
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
        if run.feedback.is_some() {
            "REVIEW"
        } else {
            "A:ANSWER B:LEAVE"
        },
        if run.feedback.is_some() { AMBER } else { MIST },
        1,
    );

    let Some(cart) = state.cartridge.as_ref() else {
        return;
    };
    let Some(question) = cart.questions.get(run.question) else {
        return;
    };
    frame.wrapped_compact_text(
        28,
        35,
        &question.question,
        PARCH,
        QUIZ_QUESTION_COLUMNS,
        QUIZ_QUESTION_ROWS,
    );

    for (index, choice) in question.choices.iter().take(4).enumerate() {
        let y = 69 + index as i32 * 22;
        let focused = run.selected == index;
        if focused {
            draw_asset_focus(frame, 23, y, 204, 20);
        }
        let mut color = if focused { PARCH } else { MIST };
        let mut label = "";
        if let Some((correct, _)) = run.feedback {
            if index == question.answer {
                color = GREEN;
                label = "CORRECT";
            } else if focused && !correct {
                color = RED;
                label = "WRONG";
            }
        }
        let max_choice = if label.is_empty() {
            QUIZ_CHOICE_CHARS
        } else {
            24
        };
        frame.compact_text(31, y + 7, &truncate(choice, max_choice), color);
        if !label.is_empty() {
            frame.text(173, y + 7, label, color, 1);
        }
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
        frame.centered_text(57, &format!("SCORE {:04}", run.score), GOLD, 1);
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
    draw_weapon(frame, x, y, scale, state.hero_class, accent);
    draw_crab(frame, x, y, scale);
    frame.rect(x + 8 * scale, y + 8 * scale, 12 * scale, 3 * scale, accent);
    draw_accessory(frame, x, y, scale, state.hero_name, accent);
}

fn draw_oracle_hero(frame: &mut Framebuffer, x: i32, y: i32, scale: i32, state: &GameState) {
    let accent = match state.hero_style {
        0 => MAGENTA,
        1 => CYAN,
        2 => GREEN,
        3 => AMBER,
        _ => VIOLET,
    };
    frame.blit_rgba(
        ORACLE_HEROES[state.hero_style],
        HERO_SPRITE_WIDTH,
        HERO_SPRITE_HEIGHT,
        x,
        y,
        scale,
    );
    draw_oracle_weapon(frame, x, y, scale, state.hero_class, accent);
    draw_oracle_accessory(frame, x, y, scale, state.hero_name, accent);
}

fn draw_oracle_weapon(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    scale: i32,
    weapon: usize,
    accent: Color,
) {
    let edge_x = x + 25 * scale;
    match weapon {
        0 => {
            frame.rect(edge_x, y + 10 * scale, scale, 12 * scale, PARCH);
            frame.rect(edge_x - 2 * scale, y + 20 * scale, 5 * scale, scale, AMBER);
        }
        1 => {
            frame.rect(edge_x, y + 12 * scale, scale, 13 * scale, AMBER);
            frame.rect(
                edge_x - 2 * scale,
                y + 8 * scale,
                5 * scale,
                5 * scale,
                accent,
            );
            frame.pixel(edge_x, y + 10 * scale, PARCH);
        }
        2 => {
            frame.line(
                x - 4 * scale,
                y + 18 * scale,
                x + 2 * scale,
                y + 18 * scale,
                accent,
            );
            frame.line(
                x - 4 * scale,
                y + 18 * scale,
                x - 4 * scale,
                y + 23 * scale,
                accent,
            );
            frame.line(x - 4 * scale, y + 23 * scale, x, y + 23 * scale, PARCH);
        }
        3 => {
            frame.rect(edge_x, y + 13 * scale, 2 * scale, 12 * scale, AMBER);
            frame.rect(
                edge_x - 3 * scale,
                y + 9 * scale,
                7 * scale,
                5 * scale,
                accent,
            );
        }
        4 => {
            frame.line(edge_x, y + 8 * scale, edge_x, y + 25 * scale, AMBER);
            frame.line(
                edge_x,
                y + 8 * scale,
                edge_x + 4 * scale,
                y + 16 * scale,
                accent,
            );
            frame.line(
                edge_x + 4 * scale,
                y + 16 * scale,
                edge_x,
                y + 25 * scale,
                accent,
            );
        }
        _ => {
            frame.outline(x - 5 * scale, y + 13 * scale, 6 * scale, 11 * scale, accent);
            frame.rect(x - 3 * scale, y + 16 * scale, 2 * scale, 5 * scale, AMBER);
        }
    }
}

fn draw_oracle_accessory(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    scale: i32,
    accessory: usize,
    accent: Color,
) {
    match accessory {
        0 => {
            frame.rect(x + 8 * scale, y + 13 * scale, 3 * scale, scale, INK);
            frame.rect(x + 13 * scale, y + 13 * scale, 3 * scale, scale, INK);
        }
        1 => {
            frame.rect(x + 4 * scale, y + 2 * scale, 16 * scale, scale, AMBER);
            frame.rect(x + 8 * scale, y, 9 * scale, 3 * scale, accent);
        }
        2 => {
            frame.rect(x + 8 * scale, y + 20 * scale, 3 * scale, 3 * scale, accent);
            frame.rect(x + 13 * scale, y + 20 * scale, 3 * scale, 3 * scale, accent);
        }
        3 => {
            frame.outline(x + 13 * scale, y + 10 * scale, 5 * scale, 5 * scale, AMBER);
            frame.line(
                x + 17 * scale,
                y + 14 * scale,
                x + 19 * scale,
                y + 18 * scale,
                AMBER,
            );
        }
        4 => {
            for offset in [0, 6, 12] {
                frame.rect(
                    x + (5 + offset) * scale,
                    y + 2 * scale,
                    3 * scale,
                    3 * scale,
                    AMBER,
                );
            }
        }
        _ => {
            frame.rect(x + 5 * scale, y + 10 * scale, 14 * scale, 3 * scale, INK);
            frame.pixel(x + 8 * scale, y + 11 * scale, accent);
            frame.pixel(x + 15 * scale, y + 11 * scale, accent);
        }
    }
}

fn draw_defeated_hero(frame: &mut Framebuffer, x: i32, y: i32, state: &GameState) {
    draw_hero(frame, x, y, 1, state);
    frame.rect(x + 5, y + 12, 14, 2, VOID);
    frame.line(x + 4, y + 35, x + 20, y + 35, ASH);
    frame.line(x - 7, y + 36, x + 31, y + 36, INDIGO);
}

fn draw_weapon(frame: &mut Framebuffer, x: i32, y: i32, scale: i32, weapon: usize, accent: Color) {
    match weapon {
        0 => {
            frame.rect(x + 31 * scale, y - 6 * scale, 2 * scale, 17 * scale, PARCH);
            frame.rect(x + 29 * scale, y - 8 * scale, 6 * scale, 3 * scale, PARCH);
            frame.rect(x + 28 * scale, y + 8 * scale, 7 * scale, 2 * scale, GOLD);
            frame.rect(x + 31 * scale, y + 10 * scale, 2 * scale, 6 * scale, accent);
        }
        1 => {
            frame.rect(x + 31 * scale, y - 3 * scale, 2 * scale, 20 * scale, GOLD);
            frame.rect(x + 28 * scale, y - 9 * scale, 8 * scale, 7 * scale, accent);
            frame.rect(x + 30 * scale, y - 7 * scale, 4 * scale, 3 * scale, PARCH);
        }
        2 => {
            frame.rect(x - 7 * scale, y + 2 * scale, 10 * scale, 3 * scale, accent);
            frame.rect(x - 7 * scale, y + 4 * scale, 3 * scale, 8 * scale, accent);
            frame.rect(x - 4 * scale, y + 9 * scale, 7 * scale, 3 * scale, accent);
            frame.rect(x - 9 * scale, y, 3 * scale, 3 * scale, MIST);
        }
        3 => {
            frame.rect(x + 31 * scale, y + scale, 3 * scale, 17 * scale, GOLD);
            frame.rect(x + 27 * scale, y - 5 * scale, 11 * scale, 7 * scale, accent);
            frame.rect(x + 29 * scale, y - 7 * scale, 7 * scale, 2 * scale, PARCH);
        }
        4 => {
            frame.rect(x + 31 * scale, y - 6 * scale, 2 * scale, 23 * scale, GOLD);
            frame.rect(x + 28 * scale, y - 4 * scale, 3 * scale, 5 * scale, accent);
            frame.rect(x + 27 * scale, y + scale, 3 * scale, 8 * scale, accent);
            frame.rect(x + 28 * scale, y + 9 * scale, 3 * scale, 5 * scale, accent);
            frame.rect(x + 26 * scale, y + 4 * scale, 9 * scale, scale, PARCH);
        }
        _ => {
            frame.rect(x - 8 * scale, y + 2 * scale, 11 * scale, 13 * scale, accent);
            frame.rect(x - 6 * scale, y + 4 * scale, 7 * scale, 9 * scale, GOLD);
            frame.rect(x - 4 * scale, y + 6 * scale, 3 * scale, 5 * scale, accent);
        }
    }
}

fn draw_accessory(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    scale: i32,
    accessory: usize,
    accent: Color,
) {
    match accessory {
        0 => {
            frame.rect(x + 8 * scale, y + 7 * scale, 5 * scale, 3 * scale, INK);
            frame.rect(x + 15 * scale, y + 7 * scale, 5 * scale, 3 * scale, INK);
            frame.rect(x + 12 * scale, y + 8 * scale, 4 * scale, 2 * scale, INK);
        }
        1 => {
            frame.rect(x + 3 * scale, y - 4 * scale, 22 * scale, 3 * scale, INK);
            frame.rect(x + 8 * scale, y - 10 * scale, 14 * scale, 7 * scale, accent);
            frame.rect(x + 8 * scale, y - 4 * scale, 14 * scale, 2 * scale, GOLD);
        }
        2 => {
            frame.rect(x + 8 * scale, y + 9 * scale, 5 * scale, 5 * scale, accent);
            frame.rect(x + 15 * scale, y + 9 * scale, 5 * scale, 5 * scale, accent);
            frame.rect(x + 13 * scale, y + 10 * scale, 2 * scale, 3 * scale, GOLD);
        }
        3 => {
            frame.outline(x + 16 * scale, y + scale, 7 * scale, 7 * scale, GOLD);
            frame.rect(x + 22 * scale, y + 7 * scale, 2 * scale, 8 * scale, GOLD);
        }
        4 => {
            frame.rect(x + 5 * scale, y - 4 * scale, 18 * scale, 4 * scale, GOLD);
            frame.rect(x + 5 * scale, y - 8 * scale, 4 * scale, 5 * scale, GOLD);
            frame.rect(x + 12 * scale, y - 10 * scale, 4 * scale, 7 * scale, accent);
            frame.rect(x + 19 * scale, y - 8 * scale, 4 * scale, 5 * scale, GOLD);
        }
        _ => {
            frame.rect(x + 5 * scale, y + 2 * scale, 18 * scale, 5 * scale, INK);
            frame.rect(x + 7 * scale, y + 3 * scale, 5 * scale, 2 * scale, accent);
            frame.rect(x + 16 * scale, y + 3 * scale, 5 * scale, 2 * scale, accent);
            frame.rect(x + 12 * scale, y + 3 * scale, 4 * scale, 2 * scale, GOLD);
        }
    }
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

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
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
            }],
            question_batch_ends: Vec::new(),
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
        ppm.extend(frame.chunks_exact(4).flat_map(|pixel| &pixel[..3]).copied());
        std::fs::write(directory.join(format!("{name}.ppm")), ppm)
            .expect("preview should be writable");
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
            .chunks_exact(4)
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
        let menu_panel = ui_box_bounds(GATEWAY_MENU_OPTION_BOXES[0]);
        let menu_heading = ui_box_bounds(GATEWAY_MENU_HEADING_BOX);
        let menu_subtitle = ui_box_bounds(GATEWAY_MENU_SUBTITLE_BOX);
        let atelier_row = LayoutBounds {
            x: 116,
            y: 17,
            width: 98,
            height: 29,
        };
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
        let atelier_accessory = ui_box_bounds(ATELIER_ACCESSORY_BOX);
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
                menu_panel,
                centered_text_in_bounds(menu_panel, 94, "RETURN TO TITLE", 1),
            ),
            (
                "menu detail",
                menu_panel,
                centered_text_in_bounds(menu_panel, 102, "CLOSE ARCHIVE", 1),
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
                centered_text_box_bounds(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER", 1),
            ),
            (
                "atelier accessory",
                atelier_accessory,
                centered_text_box_bounds(ATELIER_ACCESSORY_BOX, "MUSTACHE", 1),
            ),
            (
                "atelier row label",
                atelier_row,
                text_bounds(126, 23, "PATH", 1),
            ),
            (
                "atelier longest value",
                atelier_row,
                compact_text_bounds(122, 33, "<MERGE PALADIN>"),
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
                compact_text_bounds(28, 35, &"Q".repeat(QUIZ_QUESTION_COLUMNS)),
            ),
            (
                "trial number",
                trial_header,
                text_bounds(44, 7, "TRIAL 99", 1),
            ),
            (
                "trial hearts",
                trial_header,
                text_bounds(151, 7, "HP ***", 1),
            ),
            ("trial score", trial_header, text_bounds(215, 7, "9999", 1)),
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
                compact_text_bounds(31, 76, &"C".repeat(QUIZ_CHOICE_CHARS)),
            ),
            (
                "quiz feedback choice",
                quiz_choice,
                compact_text_bounds(31, 76, &"C".repeat(24)),
            ),
            (
                "quiz feedback label",
                quiz_choice,
                text_bounds(173, 76, "CORRECT", 1),
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
                "atelier heading",
                atelier_header,
                centered_text_box_bounds(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER", 1),
            ),
            (
                "atelier accessory",
                atelier_accessory,
                centered_text_box_bounds(ATELIER_ACCESSORY_BOX, "MUSTACHE", 1),
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
                "menu option",
                menu_panel,
                centered_text_in_bounds(menu_panel, 94, "RETURN TO TITLE", 1),
            ),
            (
                "menu detail",
                menu_panel,
                centered_text_in_bounds(menu_panel, 102, "CLOSE ARCHIVE", 1),
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
                "menu label and detail",
                centered_text_in_bounds(menu_panel, 94, "RETURN TO TITLE", 1),
                centered_text_in_bounds(menu_panel, 102, "CLOSE ARCHIVE", 1),
            ),
            (
                "atelier label and value",
                text_bounds(126, 23, "PATH", 1),
                compact_text_bounds(122, 33, "<MERGE PALADIN>"),
            ),
            (
                "atelier status and controls",
                text_bounds(5, 148, "VISION CLOUDY - RETRYING", 1),
                text_bounds(174, 148, "START:BIND", 1),
            ),
            (
                "quiz choice and result",
                compact_text_bounds(31, 76, &"C".repeat(24)),
                text_bounds(173, 76, "CORRECT", 1),
            ),
            (
                "trial tier and controls",
                text_bounds(44, 20, "ORACLE-BOUND", 1),
                text_bounds(126, 20, "A:ANSWER B:LEAVE", 1),
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

        let data_pixels = color_pixels_in_region(engine.frame(), GREEN, 0..WIDTH, 40..120);
        let bug_pixels = color_pixels_in_region(engine.frame(), RED, 0..WIDTH, 40..120);
        assert!(data_pixels > 0, "no collectible data appeared");
        assert!(bug_pixels > 0, "no bug appeared");
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
        for (label, color) in [("Oracle title/progress", GOLD), ("Claude status", SKY)] {
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
    fn quiz_waits_for_claude_before_entering_play() {
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
                selected: 0,
                hearts: 3,
                score: 100,
                level: 1,
                streak: 1,
                leveled_up: false,
                feedback: None,
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
    fn character_choices_change_accessory_weapon_and_style_pixels() {
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

        let default_accessory = hero_pixels(&engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_ne!(hero_pixels(&engine), default_accessory);
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
        let default_weapon = hero_pixels(&engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_ne!(hero_pixels(&engine), default_weapon);
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
    fn empty_quiz_cartridge_requests_the_first_claude_batch() {
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
    fn oracle_retries_a_failed_claude_batch_while_waiting() {
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
    fn failed_prefetch_waits_before_requesting_claude_again() {
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
    fn surviving_a_complete_claude_batch_levels_up() {
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
                questions: vec![question; QUESTION_BATCH_SIZE],
            },
        );
        for _ in 0..75 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Quiz);

        for (index, wrong) in [true, false, false, true, false, false]
            .into_iter()
            .enumerate()
        {
            if wrong {
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
                EngineCommand::Input {
                    button: Button::A,
                    pressed: false,
                },
            );
            for _ in 0..45 {
                engine.update();
            }
            if index + 1 < QUESTION_BATCH_SIZE {
                assert_eq!(engine.screen(), Screen::Quiz);
            }
        }

        assert_eq!(engine.screen(), Screen::LevelUp);
        let run = engine
            .app
            .world()
            .resource::<GameState>()
            .quiz
            .as_ref()
            .unwrap();
        assert_eq!(run.level, 2);
        assert_eq!(run.hearts, 1);
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
            for button in [Button::Down, Button::A] {
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
            for _ in 0..45 {
                engine.update();
            }
        }

        assert_eq!(engine.screen(), Screen::GameOver);
        let state = engine.app.world().resource::<GameState>();
        let run = state.quiz.as_ref().unwrap();
        assert_eq!(run.level, 1);
        assert_eq!(run.completed_batches, 0);
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
            for _ in 0..45 {
                engine.update();
            }
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

        for _ in 0..180 {
            state.tick_machine();
        }
        assert_eq!(state.screen, Screen::OpeningFanfare);
        render_opening_fanfare(&mut frame, &state);
        let opening_bright = color_pixels_in_region(&frame.pixels, CYAN, 0..WIDTH, 0..HEIGHT)
            + color_pixels_in_region(&frame.pixels, AMBER, 0..WIDTH, 0..HEIGHT);
        assert_eq!(opening_bright, 0, "the fanfare also starts dormant");
        let opening_luminance = total_luminance(&frame.pixels);

        state.screen_ticks = 96;
        render_opening_fanfare(&mut frame, &state);
        let cyan_signal_luminance = total_luminance(&frame.pixels);

        state.screen_ticks = 168;
        render_opening_fanfare(&mut frame, &state);
        let convergence_luminance = total_luminance(&frame.pixels);

        state.screen_ticks = 260;
        render_opening_fanfare(&mut frame, &state);
        let crescendo_luminance = total_luminance(&frame.pixels);
        assert!(
            opening_luminance < cyan_signal_luminance
                && cyan_signal_luminance < convergence_luminance
                && convergence_luminance < crescendo_luminance,
            "the Oracle luminance must climb through dormant, cyan signal, convergence, and crescendo"
        );
    }

    #[test]
    fn oracle_templates_produce_nine_distinct_native_scene_frames() {
        let mut state = GameState {
            cartridge: Some(oracle_template_cartridge()),
            quiz: Some(QuizRun {
                question: 0,
                completed_batches: 3,
                selected: 0,
                hearts: 2,
                score: 420,
                level: 4,
                streak: 3,
                leveled_up: true,
                feedback: None,
            }),
            questions_loading: true,
            screen_ticks: 90,
            ..Default::default()
        };

        let mut previews = Vec::new();
        state.screen_ticks = 60;
        let mut boot = Framebuffer::default();
        render_boot(&mut boot, &state);
        maybe_write_preview("00-boot", &boot.pixels);
        for (name, ticks) in [
            ("02a-awakening-dormant", 0),
            ("02b-awakening-cyan", 96),
            ("02c-awakening-convergence", 168),
            ("02d-awakening-crescendo", 260),
        ] {
            state.screen_ticks = ticks;
            let mut frame = Framebuffer::default();
            render_oracle_awakening(&mut frame, &state);
            maybe_write_preview(name, &frame.pixels);
        }
        for (name, renderer) in [
            (
                "01-chronicle",
                render_oracle_chronicle as fn(&mut Framebuffer, &GameState),
            ),
            ("02-awakening", render_oracle_awakening),
            ("03-title", render_oracle_title),
            ("04-menu", render_oracle_menu),
            ("05-atelier", render_oracle_atelier),
            ("06-sanctum", render_oracle_sanctum),
            ("07-trial", render_oracle_trial),
            ("08-ascension", render_oracle_ascension),
            ("09-aftermath", render_oracle_aftermath),
        ] {
            state.screen_ticks = match name {
                "02-awakening" => 260,
                "03-title" => 60,
                _ => 90,
            };
            let mut frame = Framebuffer::default();
            renderer(&mut frame, &state);
            maybe_write_preview(name, &frame.pixels);
            previews.push(frame.pixels);
        }

        let distinct = previews.iter().collect::<HashSet<_>>();
        assert_eq!(
            distinct.len(),
            9,
            "every reachable scene needs its own authored composition"
        );
        assert!(previews.iter().all(|frame| frame.len() == FRAME_BYTES));
    }

    #[test]
    fn oracle_progression_changes_the_sanctum_without_relying_on_level_text() {
        let mut state = GameState {
            cartridge: Some(oracle_template_cartridge()),
            quiz: Some(QuizRun {
                question: 0,
                completed_batches: 0,
                selected: 0,
                hearts: 3,
                score: 0,
                level: 1,
                streak: 0,
                leveled_up: false,
                feedback: None,
            }),
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

    #[test]
    fn wrapping_never_splits_into_oversized_lines() {
        let lines = wrap_text("alpha beta supercalifragilistic", 8);
        assert!(lines.iter().all(|line| line.chars().count() <= 8));
    }
}
