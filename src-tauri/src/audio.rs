//! Engine-owned chip sound for the CODE QUEST ADVANCE handheld.
//!
//! Sound is a pure function of observable state transitions, exactly like
//! rendering. The engine builds an [`AudioSnapshot`] every tick and the
//! [`AudioDirector`] diffs it against the previous tick to choose one-shot cues,
//! scene-owned loops, and scripted opening beats. Gameplay code never calls
//! audio, and the shell speaker never decides what to play.
//!
//! The output describes a constrained four-voice chip: two pulse voices with
//! selectable duty, a triangle-like wave voice, and a noise voice. Every
//! [`Note`] carries the absolute engine tick on which it starts. Voices are
//! monophonic: a note replaces whatever its voice was sounding, and a
//! zero-volume note is a cut. That rule lets the director end a scene's loop on
//! the exact tick the scene exits and keeps one cue from stacking over another.

use std::collections::VecDeque;

use bevy::prelude::Resource;
use serde::Serialize;

/// Notes the transport holds for the shell before dropping the oldest.
pub const AUDIO_QUEUE_CAPACITY: usize = 512;
/// Scene loops and beds never exceed this volume, so every cue reads above them.
pub const AMBIENCE_MAX_VOLUME: u8 = 5;

/// Engine ticks, relative to scene entry, on which the chronicle reveals its
/// header, notice, authors, and archive dates.
const ARCHIVE_REVEAL_TICKS: [u16; 4] = [1, 12, 24, 42];
/// Level-up celebrates after the batch-complete telegraph has landed.
const REWARD_DELAY: u16 = 18;
/// The fullest level-up cadence ends by the reward's 60-tick input hold.
const REWARD_STEP: u16 = 5;
/// Game-over holds the final score under a quiet chord after the defeat fall.
const SCORE_HOLD_DELAY: u16 = 54;

/// One chip voice. Each voice sounds at most one note at a time.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Voice {
    Pulse1,
    Pulse2,
    Wave,
    Noise,
}

use Voice::{Noise, Pulse1, Pulse2, Wave};

impl Voice {
    pub const ALL: [Self; 4] = [Pulse1, Pulse2, Wave, Noise];

    fn index(self) -> usize {
        self as usize
    }
}

/// Pulse duty cycle. The shell receives it in eighths of a period.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(into = "u8")]
pub enum Duty {
    Eighth,
    Quarter,
    Half,
    ThreeQuarters,
}

impl From<Duty> for u8 {
    fn from(duty: Duty) -> Self {
        match duty {
            Duty::Eighth => 1,
            Duty::Quarter => 2,
            Duty::Half => 4,
            Duty::ThreeQuarters => 6,
        }
    }
}

/// One scheduled chip event.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub voice: Voice,
    /// Absolute engine tick on which the note starts.
    pub tick: u64,
    pub duration_ticks: u16,
    /// MIDI note number. The noise voice reads it as brightness.
    pub pitch: u8,
    /// Chip volume 0-15. Zero cuts whatever the voice is sounding.
    pub volume: u8,
    /// Pulse duty; the wave and noise voices ignore it.
    pub duty: Duty,
    /// Ticks per one-step volume decrease; zero holds the volume.
    pub decay: u8,
    /// Semitones the pitch glides across the note; zero holds the pitch.
    pub slide: i8,
}

impl Note {
    fn cut(voice: Voice, tick: u64) -> Self {
        Self {
            voice,
            tick,
            duration_ticks: 0,
            pitch: 0,
            volume: 0,
            duty: Duty::Half,
            decay: 0,
            slide: 0,
        }
    }

    pub fn is_cut(&self) -> bool {
        self.volume == 0
    }

    fn end(&self) -> u64 {
        self.tick + u64::from(self.duration_ticks)
    }
}

/// The `engine_audio` payload: the engine's current tick and every note
/// emitted since the previous drain, oldest first.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AudioBatch {
    pub tick: u64,
    pub notes: Vec<Note>,
}

/// A bounded note queue. When the reader falls behind, the oldest notes are
/// dropped: they would already be too late to play.
#[derive(Debug, Default)]
pub struct AudioQueue {
    tick: u64,
    notes: VecDeque<Note>,
}

impl AudioQueue {
    pub fn append(&mut self, batch: AudioBatch) {
        self.tick = self.tick.max(batch.tick);
        for note in batch.notes {
            if self.notes.len() == AUDIO_QUEUE_CAPACITY {
                self.notes.pop_front();
            }
            self.notes.push_back(note);
        }
    }

    pub fn drain(&mut self) -> AudioBatch {
        AudioBatch {
            tick: self.tick,
            notes: self.notes.drain(..).collect(),
        }
    }
}

/// Bits for the device buttons held in an [`AudioSnapshot`].
pub mod pad {
    pub const UP: u16 = 1 << 0;
    pub const DOWN: u16 = 1 << 1;
    pub const LEFT: u16 = 1 << 2;
    pub const RIGHT: u16 = 1 << 3;
    pub const A: u16 = 1 << 4;
    pub const B: u16 = 1 << 5;
    pub const START: u16 = 1 << 6;
    pub const SELECT: u16 = 1 << 7;
    pub const L: u16 = 1 << 8;
    pub const R: u16 = 1 << 9;
}

/// The scene the player is in, as far as sound is concerned.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AudioScene {
    Off,
    Boot,
    Copyright,
    Opening(OpeningBeat),
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
    /// The Oracle Codex: page turns and answer reveals only, no ambience
    /// under reading.
    Codex,
    /// A scene the sound design does not know yet. It is always silent.
    #[default]
    Unlisted,
}

/// The five authored opening scenes, or the single-scene legacy fanfare.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpeningBeat {
    Legacy,
    SourceEmber,
    ArchiveAnswer,
    MemoryVault,
    Convergence,
    OracleAwakening,
}

/// The run's presentation tier; arrangements gain voices as it rises.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Tier {
    #[default]
    Initiate,
    Adept,
    OracleBound,
}

/// The truthful question-generation status the Oracle displays.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum QuestionStatus {
    #[default]
    Contacting,
    Writing,
    Retrying,
    Ready,
}

/// Where the current question is in its commit-and-review cycle.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AnswerPhase {
    #[default]
    Choosing,
    Correct,
    Wrong,
}

/// The audible parts of an active quiz run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunAudio {
    pub question: usize,
    pub selected: usize,
    pub phase: AnswerPhase,
    pub hearts: u8,
    pub multiplier: u32,
    /// Insight Runes awakened by the run's score (0-3).
    pub insight: usize,
    pub level: u32,
    pub completed_batches: usize,
    /// The committed answer redeemed a previously missed question.
    pub redeemed: bool,
    /// The lens mastery stage (1-3) the committed answer woke, if any.
    pub lens_woke: Option<u8>,
    /// A first B press has armed the leave confirmation.
    pub leave_armed: bool,
}

/// Everything sound may react to, sampled once per engine tick. New scenes and
/// fields extend this struct; the director stays silent for anything it does
/// not recognize, and a default snapshot names no scene at all.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AudioSnapshot {
    pub powered: bool,
    pub scene: AudioScene,
    pub scene_ticks: u64,
    /// Held buttons as [`pad`] bits; newly set bits are this tick's press edges.
    pub held: u16,
    pub menu_selected: usize,
    pub hero_row: usize,
    pub hero_name: usize,
    pub hero_class: usize,
    pub hero_style: usize,
    pub quest_selected: usize,
    /// The Codex page on screen (0 is the mastery overview).
    pub codex_page: usize,
    /// A pressed on a pending Codex page and revealed its sealed answer.
    pub codex_revealed: bool,
    pub run: Option<RunAudio>,
    pub data: u32,
    /// Data-charge runes lit (0-3).
    pub data_stage: usize,
    pub bugs: u32,
    /// Containment runes broken (0-3).
    pub breach_stage: usize,
    pub questions: QuestionStatus,
    pub tier: Tier,
}

impl AudioSnapshot {
    fn is_audible(&self) -> bool {
        self.powered
            && !matches!(
                self.scene,
                AudioScene::Off | AudioScene::Boot | AudioScene::Unlisted
            )
    }
}

/// A named one-shot. At most one cue starts per tick, so one input edge can
/// never produce more than one cue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cue {
    Navigate(usize),
    PageTurn(usize),
    Confirm,
    Cancel,
    Unavailable,
    Trait { row: usize, value: usize },
    BeginRun,
    OracleEnter,
    DataCollect(u32),
    ChargeRune(usize),
    BugHit,
    SealBreak(usize),
    Retry,
    Ready,
    Leave,
    QuestionReveal,
    Cursor(usize),
    Correct,
    Redeemed,
    LeaveWarning,
    FlowUp(u32),
    RuneAwaken(usize),
    LensWake(u8),
    Wrong,
    LowWard,
    WardBreak,
    BatchComplete,
    Continue(Tier),
    Defeat,
    Replay,
    Victory,
}

impl Cue {
    /// Higher priorities win when several events land on one tick.
    fn priority(self) -> u8 {
        match self {
            Self::Unavailable => 1,
            Self::Navigate(_)
            | Self::PageTurn(_)
            | Self::Cursor(_)
            | Self::Trait { .. }
            | Self::QuestionReveal => 2,
            Self::LeaveWarning => 5,
            Self::DataCollect(_) => 3,
            Self::BugHit => 4,
            Self::Retry => 5,
            Self::Ready | Self::ChargeRune(_) => 6,
            Self::SealBreak(_) => 7,
            Self::Correct | Self::Wrong => 8,
            Self::Redeemed | Self::FlowUp(_) | Self::LowWard => 9,
            Self::RuneAwaken(_) | Self::WardBreak => 10,
            Self::LensWake(_) => 11,
            Self::Confirm
            | Self::Cancel
            | Self::Leave
            | Self::BeginRun
            | Self::OracleEnter
            | Self::BatchComplete
            | Self::Continue(_)
            | Self::Defeat
            | Self::Replay
            | Self::Victory => 12,
        }
    }

    fn tones(self) -> Vec<Tone> {
        match self {
            Self::Navigate(index) => vec![tone(0, Pulse1, [E5, D5, C5, A4][index % 4], 3, 8)
                .duty(Duty::Quarter)
                .decay(1)],
            Self::Confirm => vec![
                tone(0, Pulse1, D5, 4, 10),
                tone(4, Pulse1, A5, 8, 10).decay(2),
            ],
            Self::Cancel => vec![
                tone(0, Pulse1, A4, 4, 9).duty(Duty::Quarter),
                tone(4, Pulse1, D4, 8, 9).duty(Duty::Quarter).decay(2),
            ],
            Self::Unavailable => vec![tone(0, Pulse1, BB3, 5, 6)
                .duty(Duty::Eighth)
                .decay(1)
                .slide(-1)],
            // Each identity row has its own timbre; the Oracle motif is
            // reserved for the final bind.
            Self::Trait { row: 0, value } => {
                vec![tone(0, Pulse1, PENTATONIC[value % 6], 4, 8).decay(1)]
            }
            Self::Trait { row: 1, value } => vec![tone(0, Pulse2, PENTATONIC[value % 6], 5, 8)
                .duty(Duty::ThreeQuarters)
                .decay(1)],
            Self::Trait { value, .. } => {
                vec![tone(0, Wave, PENTATONIC[value % 6], 8, 10).slide(2)]
            }
            Self::BeginRun => vec![
                tone(0, Pulse1, A4, 5, 11),
                tone(5, Pulse1, D5, 5, 11),
                tone(10, Pulse1, E5, 5, 11),
                tone(15, Pulse1, F5, 14, 11).decay(2),
                tone(15, Pulse2, A4, 14, 7).duty(Duty::Eighth).decay(2),
                tone(0, Wave, D3, 28, 9),
            ],
            Self::OracleEnter => vec![
                tone(0, Wave, A4, 16, 8).decay(2).slide(-7),
                tone(0, Pulse2, D5, 6, 6).duty(Duty::Eighth).decay(1),
            ],
            Self::DataCollect(count) => {
                vec![tone(0, Pulse1, [A5, C6, D6][count as usize % 3], 4, 8)
                    .duty(Duty::Quarter)
                    .decay(1)
                    .slide(5)]
            }
            Self::ChargeRune(stage) => {
                let run = [D5, F5, A5, D6, F6]
                    .into_iter()
                    .take(stage.clamp(1, 3) + 2)
                    .collect::<Vec<_>>();
                let mut tones = run
                    .iter()
                    .enumerate()
                    .map(|(step, pitch)| tone(step as u16 * 3, Pulse1, *pitch, 3, 10))
                    .collect::<Vec<_>>();
                let top = run[run.len() - 1];
                tones.push(
                    tone(run.len() as u16 * 3, Pulse2, top - 12, 12, 7)
                        .duty(Duty::Quarter)
                        .decay(2),
                );
                tones
            }
            Self::BugHit => vec![
                tone(0, Noise, NOISE_SNAP, 8, 9).decay(1),
                tone(0, Pulse1, C4, 8, 8).duty(Duty::Eighth).slide(-5),
            ],
            Self::SealBreak(stage) => {
                let stage = stage.clamp(1, 3) - 1;
                vec![
                    tone(0, Noise, NOISE_CRASH, 20, 11).decay(2),
                    tone(0, Pulse1, [A3, F3, D3][stage], 18, 10)
                        .duty(Duty::Eighth)
                        .decay(2)
                        .slide(-7),
                    tone(0, Wave, [D3, BB2, A2][stage], 18, 10),
                ]
            }
            Self::Retry => vec![
                tone(0, Wave, F4, 10, 9),
                tone(10, Wave, D4, 14, 9).decay(2),
                tone(0, Pulse2, A4, 24, 6)
                    .duty(Duty::Eighth)
                    .decay(4)
                    .slide(-2),
            ],
            Self::Ready => vec![
                tone(0, Pulse1, A5, 4, 10).duty(Duty::Quarter),
                tone(4, Pulse1, D6, 14, 10).duty(Duty::Quarter).decay(2),
                tone(4, Pulse2, FS5, 14, 7).duty(Duty::Quarter).decay(2),
            ],
            Self::Leave => vec![
                tone(0, Pulse1, D5, 4, 9).duty(Duty::Quarter),
                tone(4, Pulse1, A4, 4, 9).duty(Duty::Quarter),
                tone(8, Pulse1, D4, 10, 9).duty(Duty::Quarter).decay(2),
                tone(8, Wave, D3, 10, 8),
            ],
            Self::QuestionReveal => {
                vec![tone(0, Wave, D5, 6, 7), tone(6, Wave, A5, 10, 7).decay(2)]
            }
            // A dry leaf-turn: a noise swish under a soft pulse that alternates
            // pitch so paging feels directional without any ambience.
            Self::PageTurn(page) => vec![
                tone(0, Noise, NOISE_HAT, 3, 5).decay(1),
                tone(1, Pulse2, [D5, E5][page % 2], 4, 7)
                    .duty(Duty::Eighth)
                    .decay(1),
            ],
            // Two falling notes ask the question without committing to it.
            Self::LeaveWarning => vec![
                tone(0, Pulse1, E5, 4, 8).duty(Duty::Quarter),
                tone(6, Pulse1, BB4, 8, 8).duty(Duty::Quarter).decay(1),
            ],
            // The miss resolves: the correct cadence climbs past its old peak
            // to the Oracle's A, with the wave voice confirming the root.
            Self::Redeemed => commit_then(vec![
                tone(2, Pulse1, A4, 4, 11),
                tone(6, Pulse1, D5, 4, 11),
                tone(10, Pulse1, F5, 4, 11),
                tone(14, Pulse1, A5, 16, 12).decay(2),
                tone(14, Pulse2, D5, 16, 8).duty(Duty::Quarter).decay(2),
                tone(2, Wave, D3, 28, 9),
            ]),
            Self::Cursor(slot) => {
                vec![tone(0, Pulse1, [A5, G5, F5, E5][slot % 4], 3, 8).decay(1)]
            }
            Self::Correct => commit_then(vec![
                tone(2, Pulse1, E5, 5, 11),
                tone(7, Pulse1, A5, 14, 11).decay(2),
                tone(7, Pulse2, CS5, 14, 8).duty(Duty::Quarter).decay(2),
            ]),
            Self::FlowUp(multiplier) => commit_then(vec![
                tone(2, Pulse1, E5, 4, 12),
                tone(6, Pulse1, A5, 4, 12),
                tone(10, Pulse1, if multiplier >= 3 { E6 } else { CS6 }, 14, 12).decay(2),
                tone(10, Pulse2, A5, 14, 8).duty(Duty::Quarter).decay(2),
            ]),
            Self::RuneAwaken(stage) => {
                let mut tones = [A5, CS6, E6, A6, CS7]
                    .into_iter()
                    .take(stage.clamp(1, 3) + 2)
                    .enumerate()
                    .map(|(step, pitch)| tone(2 + step as u16 * 3, Pulse1, pitch, 3, 12))
                    .collect::<Vec<_>>();
                let crest = 2 + tones.len() as u16 * 3;
                tones.extend([
                    tone(crest, Pulse2, E5, 16, 8).duty(Duty::Quarter).decay(2),
                    tone(2, Wave, A4, crest + 12, 9).slide(12),
                    tone(crest, Noise, NOISE_HAT, 10, 6).decay(1),
                ]);
                commit_then(tones)
            }
            // A lens mastery rune wakes (stage 1-3), the run's loudest reward:
            // two rising notes on the Oracle's D-major crest, climbing a chord
            // tone higher with each stage, over the root and a bright shimmer.
            Self::LensWake(stage) => {
                let (low, high) =
                    [(D5, A5), (FS5, D6), (A5, FS6)][usize::from(stage.clamp(1, 3)) - 1];
                commit_then(vec![
                    tone(2, Pulse1, low, 6, 12),
                    tone(8, Pulse1, high, 20, 13).decay(3),
                    tone(8, Pulse2, low, 20, 9).duty(Duty::Quarter).decay(3),
                    tone(2, Wave, D4, 26, 10),
                    tone(8, Noise, NOISE_HAT, 8, 7).decay(1),
                ])
            }
            Self::Wrong => commit_then(wrong_buzz()),
            Self::LowWard => {
                let mut tones = wrong_buzz();
                tones.extend([tone(20, Pulse2, BB5, 4, 9), tone(26, Pulse2, BB5, 4, 9)]);
                commit_then(tones)
            }
            Self::WardBreak => commit_then(vec![
                tone(2, Noise, NOISE_CRASH, 24, 12).decay(2),
                tone(2, Pulse1, D4, 28, 11)
                    .duty(Duty::Eighth)
                    .decay(2)
                    .slide(-12),
                tone(2, Wave, D3, 28, 11).slide(-12),
            ]),
            Self::BatchComplete => {
                let mut tones = [D5, E5, F5, G5, A5]
                    .into_iter()
                    .enumerate()
                    .map(|(step, pitch)| {
                        tone(step as u16 * 3, Pulse1, pitch, 3, 11).duty(Duty::Quarter)
                    })
                    .collect::<Vec<_>>();
                tones.push(tone(0, Noise, NOISE_CRASH, 16, 7).slide(12));
                tones
            }
            Self::Continue(tier) => {
                let mut tones = vec![
                    tone(0, Pulse1, D5, 4, 10).duty(Duty::Eighth),
                    tone(4, Pulse1, A5, 10, 10).duty(Duty::Eighth).decay(2),
                ];
                if tier >= Tier::Adept {
                    tones.push(tone(4, Pulse2, F5, 10, 8).duty(Duty::Quarter).decay(2));
                }
                if tier >= Tier::OracleBound {
                    tones.push(tone(4, Wave, D4, 12, 10));
                    tones.push(tone(4, Noise, NOISE_HAT, 6, 6).decay(1));
                }
                tones
            }
            Self::Defeat => vec![
                tone(0, Noise, NOISE_CRASH, 20, 11).decay(2),
                tone(0, Pulse1, A4, 40, 11)
                    .duty(Duty::Eighth)
                    .decay(3)
                    .slide(-12),
                tone(0, Wave, D3, 14, 11),
                tone(14, Wave, A2, 14, 11),
                tone(28, Wave, D2, 20, 11).decay(2),
            ],
            // The reset cadence always uses the thin Initiate timbre, so a new
            // run audibly returns to the first tier.
            Self::Replay => vec![
                tone(0, Pulse1, D5, 4, 10).duty(Duty::Eighth),
                tone(4, Pulse1, A5, 4, 10).duty(Duty::Eighth),
                tone(8, Pulse1, D6, 12, 10).duty(Duty::Eighth).decay(2),
                tone(0, Wave, D3, 20, 8),
            ],
            Self::Victory => oracle_cadence(6),
        }
    }
}

/// A tone inside a cue or script, relative to the phrase's start.
#[derive(Clone, Copy, Debug)]
struct Tone {
    at: u16,
    voice: Voice,
    pitch: u8,
    length: u16,
    volume: u8,
    duty: Duty,
    decay: u8,
    slide: i8,
}

const fn tone(at: u16, voice: Voice, pitch: u8, length: u16, volume: u8) -> Tone {
    Tone {
        at,
        voice,
        pitch,
        length,
        volume,
        duty: Duty::Half,
        decay: 0,
        slide: 0,
    }
}

impl Tone {
    const fn duty(mut self, duty: Duty) -> Self {
        self.duty = duty;
        self
    }

    const fn decay(mut self, decay: u8) -> Self {
        self.decay = decay;
        self
    }

    const fn slide(mut self, slide: i8) -> Self {
        self.slide = slide;
        self
    }

    const fn later(mut self, ticks: u16) -> Self {
        self.at += ticks;
        self
    }

    #[cfg(test)]
    fn end(&self) -> u16 {
        self.at + self.length
    }

    fn note(self, tick: u64) -> Note {
        Note {
            voice: self.voice,
            tick,
            duration_ticks: self.length,
            pitch: self.pitch,
            volume: self.volume,
            duty: self.duty,
            decay: self.decay,
            slide: self.slide,
        }
    }
}

fn later(tones: Vec<Tone>, ticks: u16) -> Vec<Tone> {
    tones.into_iter().map(|tone| tone.later(ticks)).collect()
}

// Pitches are MIDI note numbers. The palette sits in D minor and resolves to
// D major at the Oracle's crest.
const D2: u8 = 38;
const A2: u8 = 45;
const BB2: u8 = 46;
const C3: u8 = 48;
const D3: u8 = 50;
const E3: u8 = 52;
const F3: u8 = 53;
const A3: u8 = 57;
const BB3: u8 = 58;
const C4: u8 = 60;
const D4: u8 = 62;
const E4: u8 = 64;
const F4: u8 = 65;
const G4: u8 = 67;
const A4: u8 = 69;
const BB4: u8 = 70;
const C5: u8 = 72;
const CS5: u8 = 73;
const D5: u8 = 74;
const E5: u8 = 76;
const F5: u8 = 77;
const FS5: u8 = 78;
const G5: u8 = 79;
const A5: u8 = 81;
const BB5: u8 = 82;
const C6: u8 = 84;
const CS6: u8 = 85;
const D6: u8 = 86;
const E6: u8 = 88;
const F6: u8 = 89;
const FS6: u8 = 90;
const A6: u8 = 93;
const CS7: u8 = 97;
const PENTATONIC: [u8; 6] = [D5, E5, G5, A5, C6, D6];
// Noise brightness: higher numbers are brighter and drier.
const NOISE_TICK: u8 = 100;
const NOISE_HAT: u8 = 96;
const NOISE_SNAP: u8 = 84;
const NOISE_CRASH: u8 = 72;
const NOISE_THUD: u8 = 48;
/// A loop step that leaves its voice alone.
const REST: u8 = 0;

/// Every answer commitment begins with the same dry click.
fn commit_then(mut tones: Vec<Tone>) -> Vec<Tone> {
    tones.insert(0, tone(0, Noise, NOISE_HAT, 2, 8));
    tones
}

fn wrong_buzz() -> Vec<Tone> {
    vec![
        tone(2, Pulse1, F4, 16, 10)
            .duty(Duty::Eighth)
            .decay(1)
            .slide(-2),
        tone(2, Wave, BB2, 16, 10),
    ]
}

/// The full Oracle cadence: the four-note motif over a rising bVI-bVII bass,
/// resolving to a D major crest with every voice sounding.
fn oracle_cadence(step: u16) -> Vec<Tone> {
    let crest = step * 4;
    let hold = step * 3 + 2;
    vec![
        tone(0, Pulse1, A4, step, 12),
        tone(step, Pulse1, D5, step, 12),
        tone(step * 2, Pulse1, E5, step, 12),
        tone(step * 3, Pulse1, F5, step, 12),
        tone(crest, Pulse1, FS5, hold, 12).decay(3),
        tone(0, Pulse2, F4, step * 2, 9).duty(Duty::Quarter),
        tone(step * 2, Pulse2, G4, step * 2, 9).duty(Duty::Quarter),
        tone(crest, Pulse2, A4, hold, 9)
            .duty(Duty::Quarter)
            .decay(3),
        tone(0, Wave, BB2, step * 2, 11),
        tone(step * 2, Wave, C3, step * 2, 11),
        tone(crest, Wave, D3, hold, 11).decay(3),
        tone(crest, Noise, NOISE_CRASH, hold.min(20), 8).decay(2),
    ]
}

/// The level-up celebration gains a voice with each presentation tier.
fn reward_cadence(tier: Tier) -> Vec<Tone> {
    let step = REWARD_STEP;
    if tier == Tier::OracleBound {
        return oracle_cadence(step);
    }
    let crest = step * 4;
    let hold = step * 3 + 2;
    let mut tones = vec![
        tone(0, Pulse1, A4, step, 11),
        tone(step, Pulse1, D5, step, 11),
        tone(step * 2, Pulse1, E5, step, 11),
        tone(step * 3, Pulse1, F5, step, 11),
        tone(crest, Pulse1, D5, hold, 11).decay(3),
        tone(0, Wave, D3, crest + hold, 9).decay(5),
    ];
    if tier == Tier::Adept {
        tones.extend([
            tone(0, Pulse2, F4, step * 2, 8).duty(Duty::Quarter),
            tone(step * 2, Pulse2, G4, step * 2, 8).duty(Duty::Quarter),
            tone(crest, Pulse2, A4, hold, 8)
                .duty(Duty::Quarter)
                .decay(3),
        ]);
    }
    tones
}

/// One cyan source pulse and its echo in the dormant cathedral.
fn source_pulse() -> Vec<Tone> {
    vec![
        tone(18, Pulse2, A4, 30, 6).decay(5),
        tone(54, Pulse2, A4, 20, 3).decay(5),
    ]
}

/// The altar answers the pulse a fifth above.
fn archive_answer() -> Vec<Tone> {
    vec![
        tone(0, Pulse2, A4, 12, 6).decay(2),
        tone(14, Pulse1, E5, 14, 7).duty(Duty::Quarter).decay(2),
        tone(30, Pulse2, A4, 10, 4).decay(2),
        tone(44, Pulse1, E5, 12, 4).duty(Duty::Quarter).decay(2),
    ]
}

/// Commit constellations branch as two echoed arpeggios over the vault floor.
fn memory_vault() -> Vec<Tone> {
    let mut tones = Vec::new();
    for (start, branch) in [(0, [D5, F5, A5, C6]), (32, [C5, E5, G5, A5])] {
        for (step, pitch) in branch.into_iter().enumerate() {
            let at = start + step as u16 * 5;
            let length = if step == 3 { 10 } else { 5 };
            tones.push(
                tone(at, Pulse1, pitch, length, 8)
                    .duty(Duty::Quarter)
                    .decay(1),
            );
            tones.push(
                tone(at + 3, Pulse2, pitch, length, 4)
                    .duty(Duty::Eighth)
                    .decay(1),
            );
        }
    }
    tones.push(tone(0, Wave, D3, 30, 7).decay(5));
    tones.push(tone(32, Wave, C3, 28, 7).decay(5));
    tones
}

/// Cyan descends and gold rises until both pulses meet on the same note.
fn convergence() -> Vec<Tone> {
    let mut tones = [A5, G5, F5, E5, D5, C5, BB4]
        .into_iter()
        .enumerate()
        .map(|(step, pitch)| tone(step as u16 * 6, Pulse1, pitch, 6, 8).duty(Duty::Quarter))
        .collect::<Vec<_>>();
    tones.extend(
        [D4, E4, F4, G4]
            .into_iter()
            .enumerate()
            .map(|(step, pitch)| tone(step as u16 * 10, Pulse2, pitch, 10, 7)),
    );
    tones.extend([
        tone(42, Pulse1, A4, 18, 9).duty(Duty::Quarter).decay(3),
        tone(42, Pulse2, A4, 18, 8).decay(3),
        tone(0, Wave, BB2, 30, 8),
        tone(30, Wave, C3, 30, 8).decay(5),
    ]);
    tones
}

fn opening_tones(beat: OpeningBeat) -> Vec<Tone> {
    match beat {
        OpeningBeat::SourceEmber => source_pulse(),
        OpeningBeat::ArchiveAnswer => archive_answer(),
        OpeningBeat::MemoryVault => memory_vault(),
        OpeningBeat::Convergence => convergence(),
        OpeningBeat::OracleAwakening => later(oracle_cadence(8), 4),
        // The single-scene fanfare plays the same arc against its own
        // luminance timeline.
        OpeningBeat::Legacy => [
            (0, source_pulse()),
            (40, archive_answer()),
            (110, memory_vault()),
            (180, convergence()),
            (250, later(oracle_cadence(8), 4)),
        ]
        .into_iter()
        .flat_map(|(at, tones)| later(tones, at))
        .collect(),
    }
}

/// Scene-owned one-shots scheduled at scene entry and cut if the scene exits.
fn script_tones(snapshot: &AudioSnapshot) -> Vec<Tone> {
    match snapshot.scene {
        AudioScene::Copyright => ARCHIVE_REVEAL_TICKS
            .iter()
            .map(|at| tone(*at, Noise, NOISE_TICK, 1, 4))
            .collect(),
        AudioScene::Opening(beat) => opening_tones(beat),
        AudioScene::LevelUp => later(reward_cadence(snapshot.tier), REWARD_DELAY),
        AudioScene::GameOver => vec![
            tone(SCORE_HOLD_DELAY, Wave, D3, 90, 5).decay(18),
            tone(SCORE_HOLD_DELAY, Pulse2, F4, 90, 4)
                .duty(Duty::Eighth)
                .decay(22),
        ],
        _ => Vec::new(),
    }
}

#[derive(Debug)]
struct LoopTrack {
    voice: Voice,
    /// One pitch per step; [`REST`] leaves the voice alone for that step.
    steps: &'static [u8],
    length: u16,
    volume: u8,
    duty: Duty,
    decay: u8,
}

#[derive(Debug)]
struct LoopPattern {
    step_ticks: u16,
    tracks: &'static [LoopTrack],
}

/// A restrained invitation: a slow bass and a sparse motif fragment.
static TITLE_LOOP: LoopPattern = LoopPattern {
    step_ticks: 12,
    tracks: &[
        LoopTrack {
            voice: Wave,
            steps: &[
                D3, REST, REST, REST, REST, REST, REST, REST, BB2, REST, REST, REST, REST, REST,
                REST, REST, C3, REST, REST, REST, REST, REST, REST, REST, D3, REST, REST, REST, A2,
                REST, REST, REST,
            ],
            length: 90,
            volume: 4,
            duty: Duty::Half,
            decay: 0,
        },
        LoopTrack {
            voice: Pulse2,
            steps: &[
                REST, REST, REST, REST, A4, REST, D5, REST, E5, REST, REST, REST, REST, REST, REST,
                REST, REST, REST, REST, REST, F5, REST, E5, REST, D5, REST, REST, REST, REST, REST,
                REST, REST,
            ],
            length: 20,
            volume: 3,
            duty: Duty::Eighth,
            decay: 5,
        },
    ],
};

/// The quieter bed under the menu and the hero atelier.
static MENU_LOOP: LoopPattern = LoopPattern {
    step_ticks: 12,
    tracks: &[
        LoopTrack {
            voice: Wave,
            steps: &[
                D3, REST, REST, REST, A2, REST, REST, REST, BB2, REST, REST, REST, C3, REST, REST,
                REST,
            ],
            length: 44,
            volume: 3,
            duty: Duty::Half,
            decay: 0,
        },
        LoopTrack {
            voice: Pulse2,
            steps: &[
                REST, REST, A4, REST, REST, REST, F4, REST, REST, REST, G4, REST, REST, REST, E4,
                REST,
            ],
            length: 14,
            volume: 2,
            duty: Duty::Eighth,
            decay: 3,
        },
    ],
};

const DATAFALL_BASS: LoopTrack = LoopTrack {
    voice: Wave,
    steps: &[
        D3, REST, REST, REST, REST, REST, REST, REST, C3, REST, REST, REST, BB2, REST, REST, REST,
    ],
    length: 36,
    volume: 4,
    duty: Duty::Half,
    decay: 0,
};

const DATAFALL_PULSE: LoopTrack = LoopTrack {
    voice: Pulse2,
    steps: &[
        REST, REST, REST, REST, A5, REST, REST, REST, REST, REST, REST, REST, REST, REST, REST,
        REST,
    ],
    length: 8,
    volume: 3,
    duty: Duty::Quarter,
    decay: 2,
};

const DATAFALL_DOUBLE_PULSE: LoopTrack = LoopTrack {
    steps: &[
        REST, REST, REST, REST, A5, REST, REST, REST, REST, REST, REST, REST, D6, REST, REST, REST,
    ],
    ..DATAFALL_PULSE
};

const DATAFALL_COUNTER: LoopTrack = LoopTrack {
    voice: Pulse1,
    steps: &[
        REST, REST, D5, REST, REST, E5, REST, REST, REST, REST, F5, REST, REST, E5, REST, REST,
    ],
    length: 8,
    volume: 2,
    duty: Duty::Eighth,
    decay: 3,
};

const DATAFALL_FULL_COUNTER: LoopTrack = LoopTrack {
    steps: &[
        REST, REST, D5, REST, REST, E5, REST, F5, REST, REST, A5, REST, REST, G5, REST, E5,
    ],
    ..DATAFALL_COUNTER
};

const DATAFALL_HATS: LoopTrack = LoopTrack {
    voice: Noise,
    steps: &[
        NOISE_HAT, REST, NOISE_HAT, REST, NOISE_HAT, REST, NOISE_HAT, NOISE_HAT, NOISE_HAT, REST,
        NOISE_HAT, REST, NOISE_HAT, REST, NOISE_HAT, REST,
    ],
    length: 2,
    volume: 2,
    duty: Duty::Half,
    decay: 1,
};

/// Initiate: a thin bass and one data pulse per bar.
static DATAFALL_INITIATE: LoopPattern = LoopPattern {
    step_ticks: 10,
    tracks: &[DATAFALL_BASS, DATAFALL_PULSE],
};

/// Adept: a second motif voice answers the data pulse.
static DATAFALL_ADEPT: LoopPattern = LoopPattern {
    step_ticks: 10,
    tracks: &[DATAFALL_BASS, DATAFALL_DOUBLE_PULSE, DATAFALL_COUNTER],
};

/// Oracle-bound: the full constrained arrangement, with noise.
static DATAFALL_ORACLE_BOUND: LoopPattern = LoopPattern {
    step_ticks: 10,
    tracks: &[
        DATAFALL_BASS,
        DATAFALL_DOUBLE_PULSE,
        DATAFALL_FULL_COUNTER,
        DATAFALL_HATS,
    ],
};

/// A low heartbeat while the run hangs on its last ward.
static DANGER_BED: LoopPattern = LoopPattern {
    step_ticks: 8,
    tracks: &[
        LoopTrack {
            voice: Wave,
            steps: &[A2, A2, REST, REST, REST, REST, REST, REST],
            length: 5,
            volume: 4,
            duty: Duty::Half,
            decay: 1,
        },
        LoopTrack {
            voice: Noise,
            steps: &[NOISE_THUD, NOISE_THUD, REST, REST, REST, REST, REST, REST],
            length: 3,
            volume: 2,
            duty: Duty::Half,
            decay: 1,
        },
    ],
};

static BATTLE_LOOP: LoopPattern = LoopPattern {
    step_ticks: 10,
    tracks: &[
        LoopTrack {
            voice: Wave,
            steps: &[D3, REST, D3, REST, F3, REST, E3, REST],
            length: 8,
            volume: 3,
            duty: Duty::Half,
            decay: 1,
        },
        LoopTrack {
            voice: Noise,
            steps: &[REST, REST, REST, REST, NOISE_SNAP, REST, REST, REST],
            length: 2,
            volume: 2,
            duty: Duty::Half,
            decay: 1,
        },
    ],
};

/// The loop a scene owns and how many ticks after entry it begins.
fn scene_loop(snapshot: &AudioSnapshot) -> Option<(&'static LoopPattern, u64)> {
    match snapshot.scene {
        AudioScene::Title => Some((&TITLE_LOOP, 30)),
        AudioScene::QuizMenu | AudioScene::CharacterCreation | AudioScene::QuestSelect => {
            Some((&MENU_LOOP, 16))
        }
        AudioScene::Oracle => Some((
            match snapshot.tier {
                Tier::Initiate => &DATAFALL_INITIATE,
                Tier::Adept => &DATAFALL_ADEPT,
                Tier::OracleBound => &DATAFALL_ORACLE_BOUND,
            },
            32,
        )),
        AudioScene::Quiz if snapshot.run.is_some_and(|run| run.hearts == 1) => {
            Some((&DANGER_BED, 24))
        }
        AudioScene::Battle => Some((&BATTLE_LOOP, 8)),
        _ => None,
    }
}

/// The cue that announces a scene change, keyed by where the player came from.
fn entry_cue(before: &AudioSnapshot, after: &AudioSnapshot) -> Option<Cue> {
    use AudioScene as Scene;
    if !before.is_audible() {
        return None;
    }
    match (before.scene, after.scene) {
        (Scene::Title, Scene::QuizMenu | Scene::QuestSelect)
        | (Scene::QuizMenu, Scene::CharacterCreation | Scene::Codex)
        | (Scene::QuestSelect, Scene::Battle)
        | (Scene::Victory | Scene::Defeat, _) => Some(Cue::Confirm),
        (Scene::QuizMenu | Scene::QuestSelect, Scene::Title)
        | (Scene::CharacterCreation | Scene::Quiz | Scene::Codex, Scene::QuizMenu) => {
            Some(Cue::Cancel)
        }
        (Scene::CharacterCreation, Scene::Oracle | Scene::Quiz) => Some(Cue::BeginRun),
        (Scene::Oracle, Scene::Quiz) => Some(Cue::QuestionReveal),
        (Scene::Oracle, Scene::QuizMenu) => Some(Cue::Leave),
        (Scene::Quiz, Scene::Oracle) => Some(Cue::OracleEnter),
        (Scene::Quiz, Scene::LevelUp) => Some(Cue::BatchComplete),
        (Scene::Quiz, Scene::GameOver) | (Scene::Battle, Scene::Defeat) => Some(Cue::Defeat),
        (Scene::LevelUp, Scene::Quiz | Scene::Oracle) => Some(Cue::Continue(after.tier)),
        (Scene::GameOver, Scene::QuizMenu) => Some(Cue::Replay),
        (Scene::Battle, Scene::Victory) => Some(Cue::Victory),
        _ => None,
    }
}

/// The most important cue among the changes inside one continuing scene.
fn scene_event(before: &AudioSnapshot, after: &AudioSnapshot) -> Option<Cue> {
    let mut candidates = Vec::new();
    match after.scene {
        AudioScene::QuizMenu if after.menu_selected != before.menu_selected => {
            candidates.push(Cue::Navigate(after.menu_selected));
        }
        AudioScene::QuestSelect if after.quest_selected != before.quest_selected => {
            candidates.push(Cue::Navigate(after.quest_selected));
        }
        AudioScene::CharacterCreation => {
            if after.hero_row != before.hero_row {
                candidates.push(Cue::Navigate(after.hero_row));
            }
            for (row, value, previous) in [
                (0, after.hero_name, before.hero_name),
                (1, after.hero_class, before.hero_class),
                (2, after.hero_style, before.hero_style),
            ] {
                if value != previous {
                    candidates.push(Cue::Trait { row, value });
                }
            }
        }
        AudioScene::Codex if after.codex_page != before.codex_page => {
            candidates.push(Cue::PageTurn(after.codex_page));
        }
        // Revealing a sealed answer reuses the soft rising reveal motif.
        AudioScene::Codex if after.codex_revealed && !before.codex_revealed => {
            candidates.push(Cue::QuestionReveal);
        }
        AudioScene::Oracle => datafall_events(before, after, &mut candidates),
        AudioScene::Quiz => quiz_events(before, after, &mut candidates),
        _ => {}
    }
    let cue = candidates.into_iter().max_by_key(|cue| cue.priority());
    let pressed = after.held & !before.held;
    cue.or_else(|| {
        (pressed != 0 && accepts_unavailable(after, pressed)).then_some(Cue::Unavailable)
    })
}

fn datafall_events(before: &AudioSnapshot, after: &AudioSnapshot, candidates: &mut Vec<Cue>) {
    if after.data > before.data {
        candidates.push(if after.data_stage > before.data_stage {
            Cue::ChargeRune(after.data_stage)
        } else {
            Cue::DataCollect(after.data)
        });
    }
    if after.bugs > before.bugs {
        candidates.push(if after.breach_stage > before.breach_stage {
            Cue::SealBreak(after.breach_stage)
        } else {
            Cue::BugHit
        });
    }
    if after.questions != before.questions {
        match after.questions {
            QuestionStatus::Retrying => candidates.push(Cue::Retry),
            QuestionStatus::Ready => candidates.push(Cue::Ready),
            QuestionStatus::Contacting | QuestionStatus::Writing => {}
        }
    }
}

fn quiz_events(before: &AudioSnapshot, after: &AudioSnapshot, candidates: &mut Vec<Cue>) {
    let (Some(previous), Some(run)) = (before.run, after.run) else {
        return;
    };
    if run.question != previous.question {
        candidates.push(Cue::QuestionReveal);
    } else if previous.phase == AnswerPhase::Choosing && run.phase != AnswerPhase::Choosing {
        // One result cue per commit: a woken lens rune (the pedagogical
        // reward) outranks every score threshold, then the rarest one wins.
        let lens_woke = run.lens_woke.filter(|_| previous.lens_woke.is_none());
        let result = match (run.phase, lens_woke) {
            (AnswerPhase::Correct, Some(stage)) => Cue::LensWake(stage),
            (AnswerPhase::Correct, None) if run.insight > previous.insight => {
                Cue::RuneAwaken(run.insight)
            }
            (AnswerPhase::Correct, None) if run.redeemed => Cue::Redeemed,
            (AnswerPhase::Correct, None) if run.multiplier > previous.multiplier => {
                Cue::FlowUp(run.multiplier)
            }
            (AnswerPhase::Correct, None) => Cue::Correct,
            (AnswerPhase::Wrong, _) if run.hearts == 0 => Cue::WardBreak,
            (AnswerPhase::Wrong, _) if run.hearts == 1 => Cue::LowWard,
            (AnswerPhase::Wrong | AnswerPhase::Choosing, _) => Cue::Wrong,
        };
        candidates.push(result);
    } else if run.phase == AnswerPhase::Choosing && run.selected != previous.selected {
        candidates.push(Cue::Cursor(run.selected));
    } else if run.leave_armed && !previous.leave_armed {
        candidates.push(Cue::LeaveWarning);
    }
}

/// Whether a press edge that changed nothing deserves the soft unavailable cue.
/// Cinematics, answer reviews, and Datafall movement stay quiet.
fn accepts_unavailable(after: &AudioSnapshot, pressed: u16) -> bool {
    match after.scene {
        AudioScene::Title
        | AudioScene::QuizMenu
        | AudioScene::CharacterCreation
        | AudioScene::QuestSelect
        | AudioScene::LevelUp
        | AudioScene::GameOver
        | AudioScene::Codex => true,
        AudioScene::Oracle => pressed & !(pad::LEFT | pad::RIGHT) != 0,
        AudioScene::Quiz => after
            .run
            .is_some_and(|run| run.phase == AnswerPhase::Choosing),
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Owner {
    #[default]
    Idle,
    Loop,
    Cue,
    Script,
}

#[derive(Clone, Copy, Debug, Default)]
struct VoiceState {
    owner: Owner,
    sounding_until: u64,
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    note: Note,
    owner: Owner,
}

#[derive(Clone, Copy, Debug)]
struct ActiveLoop {
    pattern: &'static LoopPattern,
    anchor: u64,
}

/// Turns per-tick snapshots into notes. See the module documentation.
#[derive(Debug, Default)]
pub struct AudioDirector {
    tick: u64,
    previous: Option<AudioSnapshot>,
    voices: [VoiceState; 4],
    pending: Vec<Pending>,
    /// Ticks until which the current cue owns each voice; loops yield to it.
    cue_reserved: [u64; 4],
    /// Ticks until which the scene's script owns each voice.
    script_reserved: [u64; 4],
    scene_loop: Option<ActiveLoop>,
    last_cue: Option<Cue>,
}

impl AudioDirector {
    /// Observes one engine tick and returns the notes that start on it.
    pub fn observe(&mut self, snapshot: &AudioSnapshot) -> Vec<Note> {
        self.tick += 1;
        let now = self.tick;
        let previous = self.previous.replace(snapshot.clone());
        self.last_cue = None;
        let mut notes = Vec::new();
        if !snapshot.is_audible() {
            self.stop_scene(now, &mut notes);
            return notes;
        }

        let continuing = previous.as_ref().filter(|before| {
            before.is_audible()
                && before.scene == snapshot.scene
                && snapshot.scene_ticks >= before.scene_ticks
        });
        let cue = if let Some(before) = continuing {
            self.follow_loop(snapshot, now, &mut notes);
            scene_event(before, snapshot)
        } else {
            self.stop_scene(now, &mut notes);
            self.schedule_script(snapshot, now);
            self.scene_loop = scene_loop(snapshot).map(|(pattern, delay)| ActiveLoop {
                pattern,
                anchor: now + delay,
            });
            previous
                .as_ref()
                .and_then(|before| entry_cue(before, snapshot))
        };
        if let Some(cue) = cue {
            self.play(cue, now, &mut notes);
        }
        self.emit_pending(now, &mut notes);
        self.emit_loop(now, &mut notes);

        // A cut followed by a note on the same voice and tick is redundant.
        let sounding = notes
            .iter()
            .filter(|note| !note.is_cut())
            .map(|note| note.voice)
            .collect::<Vec<_>>();
        notes.retain(|note| !note.is_cut() || !sounding.contains(&note.voice));
        notes
    }

    /// The absolute tick of the most recent observation.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The cue that started on the most recent tick, if any.
    #[cfg(test)]
    pub fn last_cue(&self) -> Option<Cue> {
        self.last_cue
    }

    /// Ends everything the previous scene owned on this exact tick.
    fn stop_scene(&mut self, now: u64, notes: &mut Vec<Note>) {
        self.pending.clear();
        self.scene_loop = None;
        self.cue_reserved = [0; 4];
        self.script_reserved = [0; 4];
        for voice in Voice::ALL {
            self.cut(voice, now, notes);
        }
    }

    fn cut(&mut self, voice: Voice, now: u64, notes: &mut Vec<Note>) {
        let state = &mut self.voices[voice.index()];
        if state.sounding_until > now {
            notes.push(Note::cut(voice, now));
        }
        *state = VoiceState {
            owner: Owner::Idle,
            sounding_until: now.min(state.sounding_until),
        };
    }

    fn schedule_script(&mut self, snapshot: &AudioSnapshot, now: u64) {
        for tone in script_tones(snapshot) {
            let tick = now + u64::from(tone.at).saturating_sub(snapshot.scene_ticks);
            let reserved = &mut self.script_reserved[tone.voice.index()];
            *reserved = (*reserved).max(tick + u64::from(tone.length));
            self.pending.push(Pending {
                note: tone.note(tick),
                owner: Owner::Script,
            });
        }
    }

    /// Starts `cue`, silencing whatever the previous cue still had to say.
    fn play(&mut self, cue: Cue, now: u64, notes: &mut Vec<Note>) {
        self.last_cue = Some(cue);
        self.pending.retain(|pending| pending.owner != Owner::Cue);
        for voice in Voice::ALL {
            if self.voices[voice.index()].owner == Owner::Cue {
                self.cut(voice, now, notes);
            }
        }
        // The replaced cue's voices are released at once.
        self.cue_reserved = [0; 4];
        for tone in cue.tones() {
            let tick = now + u64::from(tone.at);
            let reserved = &mut self.cue_reserved[tone.voice.index()];
            *reserved = (*reserved).max(tick + u64::from(tone.length));
            self.pending.push(Pending {
                note: tone.note(tick),
                owner: Owner::Cue,
            });
        }
    }

    fn emit_pending(&mut self, now: u64, notes: &mut Vec<Note>) {
        let (due, waiting): (Vec<_>, Vec<_>) = self
            .pending
            .drain(..)
            .partition(|pending| pending.note.tick <= now);
        self.pending = waiting;
        for pending in due {
            self.voices[pending.note.voice.index()] = VoiceState {
                owner: pending.owner,
                sounding_until: pending.note.end(),
            };
            notes.push(pending.note);
        }
    }

    /// Restarts the loop when the scene's own state asks for a different bed,
    /// such as the heartbeat when the run falls to its last ward.
    fn follow_loop(&mut self, snapshot: &AudioSnapshot, now: u64, notes: &mut Vec<Note>) {
        let desired = scene_loop(snapshot);
        let unchanged = match (self.scene_loop, desired) {
            (Some(active), Some((pattern, _))) => std::ptr::eq(active.pattern, pattern),
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return;
        }
        for voice in Voice::ALL {
            if self.voices[voice.index()].owner == Owner::Loop {
                self.cut(voice, now, notes);
            }
        }
        self.scene_loop = desired.map(|(pattern, delay)| ActiveLoop {
            pattern,
            anchor: now + delay,
        });
    }

    fn emit_loop(&mut self, now: u64, notes: &mut Vec<Note>) {
        let Some(active) = self.scene_loop else {
            return;
        };
        let step_ticks = u64::from(active.pattern.step_ticks);
        let Some(elapsed) = now.checked_sub(active.anchor) else {
            return;
        };
        if !elapsed.is_multiple_of(step_ticks) {
            return;
        }
        let step = (elapsed / step_ticks) as usize;
        for track in active.pattern.tracks {
            debug_assert!(track.volume <= AMBIENCE_MAX_VOLUME);
            let pitch = track.steps[step % track.steps.len()];
            let index = track.voice.index();
            if pitch == REST || self.cue_reserved[index].max(self.script_reserved[index]) > now {
                continue;
            }
            let note = Note {
                voice: track.voice,
                tick: now,
                duration_ticks: track.length,
                pitch,
                volume: track.volume,
                duty: track.duty,
                decay: track.decay,
                slide: 0,
            };
            self.voices[index] = VoiceState {
                owner: Owner::Loop,
                sounding_until: note.end(),
            };
            notes.push(note);
        }
    }
}

/// The engine's sound output: the director plus the notes it has emitted
/// since the transport last drained them.
#[derive(Resource, Debug, Default)]
pub struct AudioOut {
    director: AudioDirector,
    queue: AudioQueue,
}

impl AudioOut {
    pub fn observe(&mut self, snapshot: &AudioSnapshot) {
        let notes = self.director.observe(snapshot);
        self.queue.append(AudioBatch {
            tick: self.director.tick(),
            notes,
        });
    }

    pub fn drain(&mut self) -> AudioBatch {
        self.queue.drain()
    }

    #[cfg(test)]
    pub fn last_cue(&self) -> Option<Cue> {
        self.director.last_cue()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn scene(scene: AudioScene) -> AudioSnapshot {
        AudioSnapshot {
            powered: true,
            scene,
            ..AudioSnapshot::default()
        }
    }

    fn quiz_scene(hearts: u8) -> AudioSnapshot {
        AudioSnapshot {
            run: Some(RunAudio {
                hearts,
                multiplier: 1,
                level: 1,
                ..RunAudio::default()
            }),
            ..scene(AudioScene::Quiz)
        }
    }

    /// Advances `snapshot` by one tick, counting scene ticks the way the
    /// engine does, and returns the notes that start on that tick.
    fn step(director: &mut AudioDirector, snapshot: &mut AudioSnapshot) -> Vec<Note> {
        snapshot.scene_ticks += 1;
        director.observe(snapshot)
    }

    fn run_for(
        director: &mut AudioDirector,
        snapshot: &mut AudioSnapshot,
        ticks: u64,
    ) -> Vec<Note> {
        (0..ticks).flat_map(|_| step(director, snapshot)).collect()
    }

    fn enter(
        director: &mut AudioDirector,
        snapshot: &mut AudioSnapshot,
        next: AudioScene,
    ) -> Vec<Note> {
        snapshot.scene = next;
        snapshot.scene_ticks = 0;
        director.observe(snapshot)
    }

    fn audible(notes: &[Note]) -> Vec<Note> {
        notes
            .iter()
            .copied()
            .filter(|note| !note.is_cut())
            .collect()
    }

    fn voices(notes: &[Note]) -> BTreeSet<Voice> {
        audible(notes).iter().map(|note| note.voice).collect()
    }

    fn tone_voices(tones: &[Tone]) -> BTreeSet<Voice> {
        tones.iter().map(|tone| tone.voice).collect()
    }

    fn every_cue() -> Vec<Cue> {
        let mut cues = vec![
            Cue::Confirm,
            Cue::Cancel,
            Cue::Unavailable,
            Cue::BeginRun,
            Cue::OracleEnter,
            Cue::BugHit,
            Cue::Retry,
            Cue::Ready,
            Cue::Leave,
            Cue::QuestionReveal,
            Cue::Correct,
            Cue::Redeemed,
            Cue::LeaveWarning,
            Cue::Wrong,
            Cue::LowWard,
            Cue::WardBreak,
            Cue::BatchComplete,
            Cue::Defeat,
            Cue::Replay,
            Cue::Victory,
        ];
        for index in 0..6 {
            cues.extend([
                Cue::Navigate(index),
                Cue::PageTurn(index),
                Cue::Cursor(index),
                Cue::DataCollect(index as u32),
            ]);
            for row in 0..3 {
                cues.push(Cue::Trait { row, value: index });
            }
        }
        for stage in 1..=3 {
            cues.extend([
                Cue::ChargeRune(stage),
                Cue::SealBreak(stage),
                Cue::RuneAwaken(stage),
                Cue::LensWake(stage as u8),
                Cue::FlowUp(stage as u32),
            ]);
        }
        for tier in [Tier::Initiate, Tier::Adept, Tier::OracleBound] {
            cues.push(Cue::Continue(tier));
        }
        cues
    }

    fn every_loop() -> [&'static LoopPattern; 7] {
        [
            &TITLE_LOOP,
            &MENU_LOOP,
            &DATAFALL_INITIATE,
            &DATAFALL_ADEPT,
            &DATAFALL_ORACLE_BOUND,
            &DANGER_BED,
            &BATTLE_LOOP,
        ]
    }

    #[test]
    fn nothing_plays_while_powered_off_or_booting() {
        let mut director = AudioDirector::default();
        let mut off = AudioSnapshot {
            scene: AudioScene::Off,
            ..AudioSnapshot::default()
        };
        assert!(run_for(&mut director, &mut off, 120).is_empty());
        off.held = pad::A | pad::START;
        assert!(run_for(&mut director, &mut off, 10).is_empty());

        let mut booting = scene(AudioScene::Boot);
        booting.held = pad::START;
        assert!(run_for(&mut director, &mut booting, 180).is_empty());

        // Even a scene that owns a loop stays silent without power.
        let mut unpowered = AudioSnapshot {
            powered: false,
            ..scene(AudioScene::Oracle)
        };
        assert!(run_for(&mut director, &mut unpowered, 180).is_empty());
    }

    #[test]
    fn power_loss_cuts_every_sounding_voice_on_that_tick() {
        let mut director = AudioDirector::default();
        let mut snapshot = AudioSnapshot {
            tier: Tier::OracleBound,
            ..scene(AudioScene::Oracle)
        };
        let before = run_for(&mut director, &mut snapshot, 120);
        assert!(
            !audible(&before).is_empty(),
            "the Datafall loop should play"
        );

        snapshot.powered = false;
        snapshot.scene = AudioScene::Off;
        let cut = step(&mut director, &mut snapshot);
        let tick = director.tick();
        assert!(cut.iter().all(|note| note.is_cut() && note.tick == tick));
        for note in before.iter().filter(|note| note.end() > tick) {
            assert!(
                cut.iter().any(|cut| cut.voice == note.voice),
                "{:?} kept sounding after power loss",
                note.voice
            );
        }
        assert!(run_for(&mut director, &mut snapshot, 240).is_empty());
    }

    #[test]
    fn unknown_scenes_are_silent_and_end_the_previous_loop() {
        let mut director = AudioDirector::default();
        let mut snapshot = scene(AudioScene::Title);
        assert!(!audible(&run_for(&mut director, &mut snapshot, 200)).is_empty());

        let exit = enter(&mut director, &mut snapshot, AudioScene::Unlisted);
        assert!(!exit.is_empty() && exit.iter().all(Note::is_cut));
        snapshot.held = pad::A;
        assert!(run_for(&mut director, &mut snapshot, 300).is_empty());
    }

    #[test]
    fn datafall_arrangement_gains_a_voice_with_each_tier() {
        let counts = [Tier::Initiate, Tier::Adept, Tier::OracleBound].map(|tier| {
            let mut director = AudioDirector::default();
            let mut snapshot = AudioSnapshot {
                tier,
                ..scene(AudioScene::Oracle)
            };
            let notes = run_for(&mut director, &mut snapshot, 400);
            assert!(notes.iter().all(|note| note.volume <= AMBIENCE_MAX_VOLUME));
            voices(&notes).len()
        });
        assert_eq!(
            counts,
            [2, 3, 4],
            "Initiate, Adept, and Oracle-bound voices"
        );
    }

    #[test]
    fn scene_loops_end_on_the_exact_tick_their_scene_exits() {
        let mut director = AudioDirector::default();
        let mut snapshot = AudioSnapshot {
            tier: Tier::OracleBound,
            ..scene(AudioScene::Oracle)
        };
        let before = audible(&run_for(&mut director, &mut snapshot, 173));
        let exit = enter(&mut director, &mut snapshot, AudioScene::QuizMenu);
        let exit_tick = director.tick();
        assert_eq!(director.last_cue(), Some(Cue::Leave));

        let tails = before
            .iter()
            .filter(|note| note.end() > exit_tick)
            .collect::<Vec<_>>();
        assert!(
            !tails.is_empty(),
            "the exit should interrupt a sounding loop"
        );
        for tail in tails {
            assert!(
                exit.iter()
                    .any(|note| note.voice == tail.voice && note.tick == exit_tick),
                "{:?} carried the Datafall loop into the menu",
                tail.voice
            );
        }

        // Until the menu loop enters, the only notes are the Leave cue's.
        let leave = Cue::Leave.tones();
        let after = run_for(&mut director, &mut snapshot, 15);
        for note in audible(&exit).iter().chain(&audible(&after)) {
            let offset = note.tick - exit_tick;
            assert!(
                leave.iter().any(|tone| u64::from(tone.at) == offset
                    && tone.voice == note.voice
                    && tone.pitch == note.pitch),
                "unexpected {note:?} after the Oracle exit"
            );
        }
    }

    #[test]
    fn simultaneous_events_start_only_the_highest_priority_cue() {
        let mut director = AudioDirector::default();
        let mut snapshot = scene(AudioScene::Oracle);
        run_for(&mut director, &mut snapshot, 40);

        snapshot.data = 3;
        snapshot.data_stage = 1;
        snapshot.bugs = 1;
        snapshot.breach_stage = 1;
        snapshot.questions = QuestionStatus::Ready;
        snapshot.held = pad::A;
        let notes = step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), Some(Cue::SealBreak(1)));
        let seal = Cue::SealBreak(1).tones();
        for note in audible(&notes) {
            assert!(
                seal.iter().any(|tone| tone.at == 0
                    && tone.voice == note.voice
                    && tone.pitch == note.pitch),
                "{note:?} stacked over the seal-break cue"
            );
        }
    }

    #[test]
    fn a_new_result_silences_the_previous_cue_instead_of_stacking() {
        let mut director = AudioDirector::default();
        let mut snapshot = quiz_scene(3);
        run_for(&mut director, &mut snapshot, 10);

        let mut run = snapshot.run.unwrap();
        run.phase = AnswerPhase::Wrong;
        run.hearts = 2;
        snapshot.run = Some(run);
        step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), Some(Cue::Wrong));
        run_for(&mut director, &mut snapshot, 3);

        // A second result a few ticks later replaces the first entirely.
        run.question = 1;
        run.phase = AnswerPhase::Choosing;
        snapshot.run = Some(run);
        step(&mut director, &mut snapshot);
        run.phase = AnswerPhase::Correct;
        snapshot.run = Some(run);
        let second = step(&mut director, &mut snapshot);
        let start = director.tick();
        assert_eq!(director.last_cue(), Some(Cue::Correct));
        let later = run_for(&mut director, &mut snapshot, 40);
        let correct = Cue::Correct.tones();
        for note in audible(&second).iter().chain(&audible(&later)) {
            assert!(
                correct
                    .iter()
                    .any(|tone| start + u64::from(tone.at) == note.tick
                        && tone.voice == note.voice
                        && tone.pitch == note.pitch),
                "{note:?} leaked from an earlier cue"
            );
        }
    }

    #[test]
    fn loops_yield_their_voices_to_a_cue_and_resume_after_it() {
        let mut director = AudioDirector::default();
        let mut snapshot = quiz_scene(1);
        // The last-ward heartbeat enters 24 ticks after the scene and beats
        // twice every 64 ticks; commit just before its second beat.
        let bed = audible(&run_for(&mut director, &mut snapshot, 31));
        assert!(bed
            .iter()
            .any(|note| note.voice == Wave && note.pitch == A2));

        snapshot.run = Some(RunAudio {
            phase: AnswerPhase::Wrong,
            ..snapshot.run.unwrap()
        });
        let later = audible(&run_for(&mut director, &mut snapshot, 90));
        let start = director.tick() - 89;
        assert_eq!(later[0].tick, start);
        let cue_end = start
            + Cue::LowWard
                .tones()
                .iter()
                .map(|tone| u64::from(tone.end()))
                .max()
                .unwrap();
        for note in &later {
            let from_bed = note.pitch == A2 || note.pitch == NOISE_THUD;
            assert!(
                !from_bed || note.tick >= cue_end || !matches!(note.voice, Wave | Noise),
                "{note:?} interrupted the low-ward cue"
            );
        }
        assert!(
            later
                .iter()
                .any(|note| note.voice == Wave && note.pitch == A2 && note.tick >= cue_end),
            "the heartbeat returns once the cue releases its voices"
        );
    }

    #[test]
    fn quiz_results_choose_the_single_most_meaningful_variant() {
        // (hearts, multiplier, insight) before -> (phase, hearts, multiplier, insight) after.
        let cases = [
            ((3, 1, 0), (AnswerPhase::Correct, 3, 1, 0), Cue::Correct),
            ((3, 1, 0), (AnswerPhase::Correct, 3, 2, 0), Cue::FlowUp(2)),
            (
                (3, 2, 0),
                (AnswerPhase::Correct, 3, 3, 1),
                Cue::RuneAwaken(1),
            ),
            ((3, 1, 0), (AnswerPhase::Wrong, 2, 1, 0), Cue::Wrong),
            ((2, 1, 0), (AnswerPhase::Wrong, 1, 1, 0), Cue::LowWard),
            ((1, 1, 0), (AnswerPhase::Wrong, 0, 1, 0), Cue::WardBreak),
        ];
        for (
            (hearts, multiplier, insight),
            (phase, hearts_after, multiplier_after, insight_after),
            expected,
        ) in cases
        {
            let mut director = AudioDirector::default();
            let mut snapshot = quiz_scene(hearts);
            let run = RunAudio {
                multiplier,
                insight,
                ..snapshot.run.unwrap()
            };
            snapshot.run = Some(run);
            run_for(&mut director, &mut snapshot, 5);
            snapshot.run = Some(RunAudio {
                phase,
                hearts: hearts_after,
                multiplier: multiplier_after,
                insight: insight_after,
                ..run
            });
            step(&mut director, &mut snapshot);
            assert_eq!(director.last_cue(), Some(expected));
        }
    }

    #[test]
    fn a_woken_lens_rune_outranks_every_score_threshold_with_one_cue() {
        // (lens stage woken, insight after, redeemed) -> the single result cue.
        let cases = [
            (Some(1), 0, false, Cue::LensWake(1)),
            (Some(2), 1, false, Cue::LensWake(2)),
            (Some(3), 0, true, Cue::LensWake(3)),
            (None, 1, false, Cue::RuneAwaken(1)),
        ];
        for (lens_woke, insight_after, redeemed, expected) in cases {
            let mut director = AudioDirector::default();
            let mut snapshot = quiz_scene(3);
            let run = RunAudio {
                multiplier: 2,
                ..snapshot.run.unwrap()
            };
            snapshot.run = Some(run);
            run_for(&mut director, &mut snapshot, 5);
            snapshot.run = Some(RunAudio {
                phase: AnswerPhase::Correct,
                multiplier: 3,
                insight: insight_after,
                redeemed,
                lens_woke,
                ..run
            });
            step(&mut director, &mut snapshot);
            assert_eq!(director.last_cue(), Some(expected));
            // The held lesson card never starts a second result cue.
            for _ in 0..60 {
                step(&mut director, &mut snapshot);
                assert_eq!(director.last_cue(), None);
            }
        }
        // Each stage climbs: the crest note rises with the stage.
        let crest = |stage: u8| {
            Cue::LensWake(stage)
                .tones()
                .iter()
                .filter(|tone| tone.voice == Pulse1)
                .map(|tone| tone.pitch)
                .max()
                .unwrap()
        };
        assert!(crest(1) < crest(2) && crest(2) < crest(3));
        let pitches = |cue: Cue| {
            cue.tones()
                .iter()
                .map(|tone| (tone.voice, tone.pitch))
                .collect::<Vec<_>>()
        };
        assert_ne!(pitches(Cue::LensWake(1)), pitches(Cue::RuneAwaken(1)));
    }

    #[test]
    fn press_edges_that_change_nothing_get_one_soft_unavailable_cue() {
        let mut director = AudioDirector::default();
        let mut snapshot = scene(AudioScene::QuizMenu);
        run_for(&mut director, &mut snapshot, 5);
        snapshot.held = pad::LEFT;
        step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), Some(Cue::Unavailable));
        // Holding the button is not a new edge.
        step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), None);

        // Datafall movement and cinematic presses stay quiet.
        let mut oracle = scene(AudioScene::Oracle);
        run_for(&mut director, &mut oracle, 5);
        oracle.held = pad::LEFT | pad::RIGHT;
        step(&mut director, &mut oracle);
        assert_eq!(director.last_cue(), None);
        oracle.held |= pad::A;
        step(&mut director, &mut oracle);
        assert_eq!(director.last_cue(), Some(Cue::Unavailable));

        let mut opening = scene(AudioScene::Opening(OpeningBeat::SourceEmber));
        run_for(&mut director, &mut opening, 5);
        opening.held = pad::START;
        step(&mut director, &mut opening);
        assert_eq!(director.last_cue(), None);
    }

    #[test]
    fn copyright_is_near_silent_apart_from_dry_archival_ticks() {
        let mut director = AudioDirector::default();
        let mut snapshot = scene(AudioScene::Boot);
        step(&mut director, &mut snapshot);
        let mut notes = enter(&mut director, &mut snapshot, AudioScene::Copyright);
        let entry = director.tick();
        notes.extend(run_for(&mut director, &mut snapshot, 179));
        assert_eq!(
            notes
                .iter()
                .map(|note| note.tick - entry)
                .collect::<Vec<_>>(),
            ARCHIVE_REVEAL_TICKS.map(u64::from).to_vec()
        );
        assert!(notes
            .iter()
            .all(|note| note.voice == Noise && note.volume <= 4 && note.duration_ticks == 1));
    }

    #[test]
    fn opening_motif_grows_from_one_pulse_to_the_full_cadence() {
        // Durations mirror the elapsed gates in the root CODEQUEST.toml.
        let beats = [
            (OpeningBeat::SourceEmber, 96),
            (OpeningBeat::ArchiveAnswer, 66),
            (OpeningBeat::MemoryVault, 66),
            (OpeningBeat::Convergence, 66),
            (OpeningBeat::OracleAwakening, 66),
        ];
        let mut previous_voices = 0;
        let mut previous_peak = 0;
        for (beat, duration) in beats {
            let tones = opening_tones(beat);
            let voice_count = tone_voices(&tones).len();
            let peak = tones.iter().map(|tone| tone.volume).max().unwrap();
            assert!(voice_count >= previous_voices, "{beat:?} lost a layer");
            assert!(
                peak >= previous_peak,
                "{beat:?} is quieter than its predecessor"
            );
            assert!(
                tones.iter().all(|tone| tone.end() <= duration - 4),
                "{beat:?} must tail cleanly before its scene ends"
            );
            previous_voices = voice_count;
            previous_peak = peak;
        }
        assert_eq!(tone_voices(&source_pulse()).len(), 1, "one source pulse");
        assert_eq!(
            tone_voices(&opening_tones(OpeningBeat::OracleAwakening)).len(),
            4,
            "the crescendo is the only full arrangement"
        );
        assert!(opening_tones(OpeningBeat::Legacy)
            .iter()
            .all(|tone| tone.end() <= 326));
    }

    #[test]
    fn skipping_the_opening_cuts_the_fanfare_at_the_title_handoff() {
        let mut director = AudioDirector::default();
        let mut snapshot = scene(AudioScene::Opening(OpeningBeat::OracleAwakening));
        director.observe(&snapshot);
        run_for(&mut director, &mut snapshot, 20);
        let handoff = enter(&mut director, &mut snapshot, AudioScene::Title);
        assert!(!handoff.is_empty() && handoff.iter().all(Note::is_cut));
        // The title loop waits for a clean breath before it begins.
        assert!(run_for(&mut director, &mut snapshot, 29).is_empty());
        assert!(!run_for(&mut director, &mut snapshot, 1).is_empty());
    }

    #[test]
    fn level_up_reward_gains_voices_by_tier_and_resolves_within_its_hold() {
        let counts = [Tier::Initiate, Tier::Adept, Tier::OracleBound].map(|tier| {
            let tones = later(reward_cadence(tier), REWARD_DELAY);
            assert!(tones.iter().all(|tone| tone.end() <= 60), "{tier:?}");
            tone_voices(&tones).len()
        });
        assert_eq!(counts, [2, 3, 4]);
        let continuation = [Tier::Initiate, Tier::Adept, Tier::OracleBound]
            .map(|tier| tone_voices(&Cue::Continue(tier).tones()).len());
        assert_eq!(continuation, [1, 2, 4]);
        assert!(Cue::BatchComplete
            .tones()
            .iter()
            .all(|tone| tone.end() <= REWARD_DELAY));
    }

    #[test]
    fn feedback_cues_read_above_every_ambience_loop() {
        for pattern in every_loop() {
            for track in pattern.tracks {
                assert!(track.volume <= AMBIENCE_MAX_VOLUME, "{track:?}");
            }
        }
        for cue in every_cue() {
            let tones = cue.tones();
            let peak = tones.iter().map(|tone| tone.volume).max().unwrap_or(0);
            assert!(
                peak > AMBIENCE_MAX_VOLUME,
                "{cue:?} is no louder than ambience"
            );
            assert!(tones.iter().all(|tone| tone.volume <= 15 && tone.pitch > 0));
        }
    }

    #[test]
    fn identity_rows_answer_with_distinct_timbres() {
        let timbres = [0, 1, 2].map(|row| {
            let tone = Cue::Trait { row, value: 1 }.tones()[0];
            (tone.voice, tone.duty, tone.slide)
        });
        assert_ne!(timbres[0], timbres[1]);
        assert_ne!(timbres[1], timbres[2]);
        assert_ne!(timbres[0], timbres[2]);
    }

    #[test]
    fn the_same_snapshots_always_produce_the_same_notes() {
        let script = |director: &mut AudioDirector| {
            let mut snapshot = scene(AudioScene::Title);
            let mut notes = run_for(director, &mut snapshot, 90);
            notes.extend(enter(director, &mut snapshot, AudioScene::QuizMenu));
            snapshot.held = pad::DOWN;
            snapshot.menu_selected = 1;
            notes.extend(run_for(director, &mut snapshot, 40));
            notes.extend(enter(director, &mut snapshot, AudioScene::Oracle));
            snapshot.data = 1;
            notes.extend(run_for(director, &mut snapshot, 200));
            notes
        };
        let first = script(&mut AudioDirector::default());
        let second = script(&mut AudioDirector::default());
        assert!(first.len() > 10);
        assert_eq!(first, second);
    }

    #[test]
    fn the_transport_queue_drops_the_oldest_notes_beyond_its_bound() {
        let mut queue = AudioQueue::default();
        let note = |tick| tone(0, Pulse1, A4, 4, 10).note(tick);
        queue.append(AudioBatch {
            tick: 700,
            notes: (0..700).map(note).collect(),
        });
        let batch = queue.drain();
        assert_eq!(batch.tick, 700);
        assert_eq!(batch.notes.len(), AUDIO_QUEUE_CAPACITY);
        assert_eq!(batch.notes[0].tick, 700 - AUDIO_QUEUE_CAPACITY as u64);
        assert_eq!(batch.notes.last().unwrap().tick, 699);
        assert_eq!(
            queue.drain(),
            AudioBatch {
                tick: 700,
                notes: Vec::new()
            }
        );
    }

    #[test]
    fn engine_audio_payload_serializes_for_the_shell_speaker() {
        let batch = AudioBatch {
            tick: 42,
            notes: vec![
                tone(0, Pulse1, A4, 8, 10)
                    .duty(Duty::Quarter)
                    .decay(2)
                    .slide(-3)
                    .note(40),
                Note::cut(Noise, 41),
            ],
        };
        assert_eq!(
            serde_json::to_value(&batch).unwrap(),
            serde_json::json!({
                "tick": 42,
                "notes": [
                    {
                        "voice": "pulse1",
                        "tick": 40,
                        "durationTicks": 8,
                        "pitch": 69,
                        "volume": 10,
                        "duty": 2,
                        "decay": 2,
                        "slide": -3
                    },
                    {
                        "voice": "noise",
                        "tick": 41,
                        "durationTicks": 0,
                        "pitch": 0,
                        "volume": 0,
                        "duty": 4,
                        "decay": 0,
                        "slide": 0
                    }
                ]
            })
        );
        for (voice, name) in Voice::ALL
            .into_iter()
            .zip(["pulse1", "pulse2", "wave", "noise"])
        {
            assert_eq!(serde_json::to_value(voice).unwrap(), name);
        }
        for (duty, eighths) in [
            (Duty::Eighth, 1),
            (Duty::Quarter, 2),
            (Duty::Half, 4),
            (Duty::ThreeQuarters, 6),
        ] {
            assert_eq!(serde_json::to_value(duty).unwrap(), eighths);
        }
    }

    #[test]
    fn datafall_cues_name_the_single_event_that_changed() {
        // (data, data_stage, bugs, breach_stage) after a settled 1/0/0/0 field.
        let cases = [
            ((2, 0, 0, 0), Cue::DataCollect(2)),
            ((2, 1, 0, 0), Cue::ChargeRune(1)),
            ((1, 0, 1, 0), Cue::BugHit),
            ((1, 0, 1, 1), Cue::SealBreak(1)),
            // A bug costs the run, so it outranks a pickup on the same tick.
            ((2, 0, 1, 0), Cue::BugHit),
        ];
        for ((data, data_stage, bugs, breach_stage), expected) in cases {
            let mut director = AudioDirector::default();
            let mut snapshot = AudioSnapshot {
                data: 1,
                ..scene(AudioScene::Oracle)
            };
            run_for(&mut director, &mut snapshot, 5);
            snapshot.data = data;
            snapshot.data_stage = data_stage;
            snapshot.bugs = bugs;
            snapshot.breach_stage = breach_stage;
            step(&mut director, &mut snapshot);
            assert_eq!(
                director.last_cue(),
                Some(expected),
                "{:?}",
                (data, data_stage, bugs, breach_stage)
            );
        }
    }

    #[test]
    fn a_question_status_is_announced_once_when_it_changes() {
        let mut director = AudioDirector::default();
        let mut snapshot = scene(AudioScene::Oracle);
        run_for(&mut director, &mut snapshot, 5);
        for (status, expected) in [
            (QuestionStatus::Writing, None),
            (QuestionStatus::Retrying, Some(Cue::Retry)),
            (QuestionStatus::Ready, Some(Cue::Ready)),
        ] {
            snapshot.questions = status;
            step(&mut director, &mut snapshot);
            assert_eq!(director.last_cue(), expected, "{status:?}");
            for _ in 0..30 {
                step(&mut director, &mut snapshot);
                assert_eq!(director.last_cue(), None, "{status:?} repeated its cue");
            }
        }
    }

    #[test]
    fn quiz_edges_start_one_cue_and_held_state_starts_none() {
        let mut director = AudioDirector::default();
        let mut snapshot = quiz_scene(3);
        run_for(&mut director, &mut snapshot, 5);

        // Redeeming a miss is rarer than a flow step, so it wins the commit.
        let mut run = snapshot.run.unwrap();
        run.phase = AnswerPhase::Correct;
        run.multiplier = 2;
        run.redeemed = true;
        snapshot.run = Some(run);
        step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), Some(Cue::Redeemed));

        // The answer review ignores cursor moves and dead presses.
        run.selected = 2;
        snapshot.run = Some(run);
        snapshot.held = pad::DOWN;
        step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), None, "the review has no cursor");
        snapshot.held = 0;
        step(&mut director, &mut snapshot);
        snapshot.held = pad::LEFT;
        step(&mut director, &mut snapshot);
        assert_eq!(
            director.last_cue(),
            None,
            "the review keeps dead presses quiet"
        );
        snapshot.held = 0;

        // A lens rune an earlier answer woke is not announced again.
        run.question = 1;
        run.phase = AnswerPhase::Choosing;
        run.redeemed = false;
        run.lens_woke = Some(1);
        snapshot.run = Some(run);
        step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), Some(Cue::QuestionReveal));
        run_for(&mut director, &mut snapshot, 5);
        run.phase = AnswerPhase::Correct;
        snapshot.run = Some(run);
        step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), Some(Cue::Correct));

        // Arming the leave confirmation warns once, however long it stays armed.
        run.question = 2;
        run.phase = AnswerPhase::Choosing;
        run.lens_woke = None;
        snapshot.run = Some(run);
        step(&mut director, &mut snapshot);
        run_for(&mut director, &mut snapshot, 5);
        run.leave_armed = true;
        snapshot.run = Some(run);
        step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), Some(Cue::LeaveWarning));
        for _ in 0..30 {
            step(&mut director, &mut snapshot);
            assert_eq!(director.last_cue(), None, "an armed leave warns once");
        }
    }

    #[test]
    fn scene_entries_announce_where_the_player_came_from() {
        use AudioScene as S;
        let cases = [
            (S::Title, S::QuizMenu, Some(Cue::Confirm)),
            (S::QuizMenu, S::Title, Some(Cue::Cancel)),
            (S::QuizMenu, S::Codex, Some(Cue::Confirm)),
            (S::Codex, S::QuizMenu, Some(Cue::Cancel)),
            (S::CharacterCreation, S::Oracle, Some(Cue::BeginRun)),
            (S::Oracle, S::Quiz, Some(Cue::QuestionReveal)),
            (S::Oracle, S::QuizMenu, Some(Cue::Leave)),
            (S::Quiz, S::Oracle, Some(Cue::OracleEnter)),
            (S::Quiz, S::LevelUp, Some(Cue::BatchComplete)),
            (S::Quiz, S::GameOver, Some(Cue::Defeat)),
            (S::GameOver, S::QuizMenu, Some(Cue::Replay)),
            (S::QuestSelect, S::Battle, Some(Cue::Confirm)),
            (S::Battle, S::Victory, Some(Cue::Victory)),
            (S::Battle, S::Defeat, Some(Cue::Defeat)),
            (S::Victory, S::QuestSelect, Some(Cue::Confirm)),
            (S::Copyright, S::Opening(OpeningBeat::SourceEmber), None),
            (S::Opening(OpeningBeat::OracleAwakening), S::Title, None),
        ];
        for (from, to, expected) in cases {
            let mut director = AudioDirector::default();
            let mut snapshot = scene(from);
            run_for(&mut director, &mut snapshot, 5);
            enter(&mut director, &mut snapshot, to);
            assert_eq!(director.last_cue(), expected, "{from:?} -> {to:?}");
        }

        // Continuing from a level-up plays the arrangement of the tier reached.
        let mut director = AudioDirector::default();
        let mut snapshot = scene(S::LevelUp);
        run_for(&mut director, &mut snapshot, 5);
        snapshot.tier = Tier::Adept;
        enter(&mut director, &mut snapshot, S::Oracle);
        assert_eq!(director.last_cue(), Some(Cue::Continue(Tier::Adept)));

        // A scene last seen without power announces nothing when power returns.
        let mut director = AudioDirector::default();
        let mut snapshot = AudioSnapshot {
            powered: false,
            ..scene(S::Defeat)
        };
        run_for(&mut director, &mut snapshot, 5);
        snapshot.powered = true;
        enter(&mut director, &mut snapshot, S::Title);
        assert_eq!(director.last_cue(), None);
    }

    #[test]
    fn re_entering_a_scene_restarts_its_script() {
        let mut director = AudioDirector::default();
        let mut snapshot = scene(AudioScene::Copyright);
        let first = run_for(&mut director, &mut snapshot, 60);
        assert_eq!(audible(&first).len(), ARCHIVE_REVEAL_TICKS.len());
        // The engine restarts the scene: its tick count falls back to zero.
        let mut again = enter(&mut director, &mut snapshot, AudioScene::Copyright);
        again.extend(run_for(&mut director, &mut snapshot, 59));
        assert_eq!(
            audible(&again).len(),
            ARCHIVE_REVEAL_TICKS.len(),
            "a restarted chronicle ticks through its reveals again"
        );
    }

    #[test]
    fn a_new_cue_cuts_every_voice_the_previous_cue_still_sounds() {
        let mut director = AudioDirector::default();
        let mut snapshot = quiz_scene(3);
        run_for(&mut director, &mut snapshot, 5);
        let mut run = snapshot.run.unwrap();
        run.phase = AnswerPhase::Wrong;
        run.hearts = 2;
        snapshot.run = Some(run);
        let mut wrong = step(&mut director, &mut snapshot);
        assert_eq!(director.last_cue(), Some(Cue::Wrong));
        wrong.extend(run_for(&mut director, &mut snapshot, 4));

        // The reveal speaks on the wave voice alone while the buzz still
        // sounds on Pulse1; the buzz must not ring on under it.
        run.question = 1;
        run.phase = AnswerPhase::Choosing;
        snapshot.run = Some(run);
        let reveal = step(&mut director, &mut snapshot);
        let tick = director.tick();
        assert_eq!(director.last_cue(), Some(Cue::QuestionReveal));
        let ringing = audible(&wrong)
            .into_iter()
            .filter(|note| note.end() > tick)
            .map(|note| note.voice)
            .collect::<BTreeSet<_>>();
        assert!(ringing.contains(&Pulse1), "the buzz is still sounding");
        for voice in ringing {
            assert!(
                reveal
                    .iter()
                    .any(|note| note.voice == voice && note.tick == tick),
                "{voice:?} kept the wrong-answer buzz under the reveal"
            );
        }
    }

    #[test]
    fn loops_reclaim_a_voice_as_soon_as_no_cue_holds_it() {
        // The Initiate bass enters 32 ticks after the scene (tick 33) and next
        // plays C3 on step 8, 80 ticks later: tick 113.
        let bass_returns = |notes: &[Note]| {
            notes
                .iter()
                .any(|note| note.voice == Wave && note.tick == 113 && note.pitch == C3)
        };
        // A seal break holds the wave voice for 18 ticks: one at tick 95 ends
        // exactly at 113, and one at 105 is replaced by a pickup at 107.
        for (seal_at, pickup_at) in [(95, None), (105, Some(107))] {
            let mut director = AudioDirector::default();
            let mut snapshot = scene(AudioScene::Oracle);
            run_for(&mut director, &mut snapshot, seal_at - 1);
            snapshot.bugs = 1;
            snapshot.breach_stage = 1;
            let mut notes = step(&mut director, &mut snapshot);
            assert_eq!(director.tick(), seal_at);
            assert_eq!(director.last_cue(), Some(Cue::SealBreak(1)));
            if let Some(pickup_at) = pickup_at {
                notes.extend(run_for(
                    &mut director,
                    &mut snapshot,
                    pickup_at - seal_at - 1,
                ));
                snapshot.data = 1;
                notes.extend(step(&mut director, &mut snapshot));
                assert_eq!(director.last_cue(), Some(Cue::DataCollect(1)));
            }
            let remaining = 120 - director.tick();
            notes.extend(run_for(&mut director, &mut snapshot, remaining));
            assert!(
                bass_returns(&notes),
                "the bass must return at tick 113 after a seal break at {seal_at}"
            );
        }
    }

    #[test]
    fn the_heartbeat_stops_on_the_tick_the_last_ward_breaks() {
        let mut director = AudioDirector::default();
        let mut snapshot = quiz_scene(1);
        // The bed enters at tick 25 and beats again at tick 33 for five ticks.
        let bed = audible(&run_for(&mut director, &mut snapshot, 34));
        assert!(bed
            .iter()
            .any(|note| note.voice == Wave && note.tick == 33 && note.end() > 35));

        snapshot.run = Some(RunAudio {
            phase: AnswerPhase::Wrong,
            hearts: 0,
            ..snapshot.run.unwrap()
        });
        let broken = step(&mut director, &mut snapshot);
        assert_eq!(director.tick(), 35);
        assert_eq!(director.last_cue(), Some(Cue::WardBreak));
        assert!(
            broken
                .iter()
                .any(|note| note.voice == Wave && note.tick == 35 && note.is_cut()),
            "the sounding heartbeat is cut when the last ward breaks"
        );
        let after = audible(&run_for(&mut director, &mut snapshot, 200));
        assert!(
            !after
                .iter()
                .any(|note| note.pitch == A2 || note.pitch == NOISE_THUD),
            "the heartbeat must not return with no ward left"
        );
    }
}
