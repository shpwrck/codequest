use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Child, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::audio::{
    self, pad, AnswerPhase, AudioBatch, AudioOut, AudioQueue, AudioScene, AudioSnapshot,
    QuestionStatus, RunAudio, Tier,
};
use crate::codequest::{CodeQuestConfig, GameType, VisualTemplate};
use crate::external_tools;
use crate::font5x7::{glyph, GLYPH_ADVANCE, GLYPH_WIDTH, LINE_HEIGHT};
use crate::learning::{
    self, AnswerEvidence, Concept, LensRecord, Lesson, Mastery, ProgressEvent, Review,
    RATIONALE_COLUMNS, RATIONALE_ROWS,
};
use crate::scene_machine::{
    SceneEvent, SceneHandler, SceneMachine, SceneMachineDefinition, SceneSignal,
};

pub const WIDTH: usize = 240;
pub const HEIGHT: usize = 160;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT * 4;
const NATIVE_RGB_BYTES: usize = WIDTH * HEIGHT * 3;

#[cfg(test)]
mod bench;
mod transcript;

use transcript::TranscriptChannel;
pub use transcript::TranscriptUpdate;

mod advance;
mod assets;
mod audio_snapshot;
mod cartridge;
mod codex;
mod commands;
mod deck;
mod framebuffer;
mod input;
mod lesson;
mod oracle;
mod palette;
mod progress;
mod quest_process;
mod quiz;
mod render;
mod runtime;
mod state;
#[cfg(test)]
mod test_support;
mod text;

use self::advance::*;
use self::assets::*;
use self::audio_snapshot::*;
use self::commands::*;
use self::deck::*;
use self::framebuffer::*;
use self::input::*;
use self::lesson::*;
use self::oracle::*;
use self::palette::Color;
use self::palette::*;
use self::progress::*;
use self::quest_process::*;
use self::quiz::*;
use self::render::aftermath::*;
use self::render::ascension::*;
use self::render::atelier::*;
use self::render::codex::*;
use self::render::datafall::*;
#[cfg(test)]
use self::render::draw_screen;
use self::render::menu::*;
use self::render::opening::*;
use self::render::quest::*;
use self::render::render;
use self::render::sprites::*;
use self::render::trial::*;
use self::render::widgets::*;
use self::state::*;
use self::text::*;

pub use self::cartridge::{
    quiz_question_fits, AnsweredQuestionRecorder, CartridgeMode, CartridgeSpec, QuestSpec,
    QuestionLoader, QuizQuestion, RepositoryProvenance, QUIZ_CHOICE_CHARS, QUIZ_QUESTION_COLUMNS,
    QUIZ_QUESTION_ROWS,
};
pub use self::input::Button;
pub use self::runtime::{EngineRuntime, GameEngine};
#[cfg(test)]
use self::test_support::*;
pub(crate) use self::text::wrap_text;
