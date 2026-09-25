//! A release-mode tick benchmark for the engine thread's 60 Hz budget.
//!
//! Each engine tick runs game logic, renders the 240x160 frame on the CPU,
//! diffs the audio snapshot, and builds the screen transcript; the runtime
//! loop then copies the frame, queues the notes, and publishes the
//! transcript. This drives a real [`GameEngine`] through the scenes a player
//! sees and reports per-tick time and heap allocations for exactly that work:
//!
//! ```text
//! cargo test --release --manifest-path src-tauri/Cargo.toml --lib \
//!   engine_tick_benchmark -- --ignored --nocapture
//! ```
//!
//! It also prints a digest of every frame and transcript it saw, so a change
//! meant to be behavior-preserving can be checked against the build before
//! it, and it holds the tests that prove the renderer's fast paths (the plate
//! cache and the awakening tables) draw exactly what their per-pixel
//! references draw.
//!
//! Allocations are counted by a test-only global allocator that forwards to
//! the system allocator and counts on the calling thread only, so parallel
//! tests do not disturb one another's numbers.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use super::transcript::TranscriptChannel;
use super::*;

struct CountingAllocator;

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

fn count_allocation() {
    let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
}

fn allocations() -> u64 {
    ALLOCATIONS.try_with(Cell::get).unwrap_or(0)
}

// SAFETY: every call forwards to the system allocator unchanged; counting
// touches only a const-initialized thread-local `Cell` that never allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

const CARTRIDGE_ID: &str = "/tmp/engine-bench";
const SAMPLES_PER_SCENE: usize = 300;

fn issue(engine: &mut GameEngine, command: EngineCommand) {
    engine.command(command);
    engine.update();
    let _ = engine.take_effects();
}

fn press(engine: &mut GameEngine, button: Button) {
    for pressed in [true, false] {
        issue(engine, EngineCommand::Input { button, pressed });
    }
}

fn state(engine: &GameEngine) -> &GameState {
    engine.app.world().resource::<GameState>()
}

fn question(index: usize) -> QuizQuestion {
    QuizQuestion {
        question: format!("WHICH LAYER DECIDES WHAT HAPPENS NEXT IN ROUND {index}?"),
        choices: vec![
            format!("THE BEVY ENGINE {index}"),
            format!("THE WEB SHELL {index}"),
            format!("THE STYLESHEET {index}"),
            format!("THE INSTALLER {index}"),
        ],
        answer: 0,
        concept: Some(Concept::Responsibility),
        rationales: vec![
            "THE ENGINE OWNS STATE, RULES, AND TIMING.".into(),
            "THE SHELL ONLY FORWARDS INPUT AND PAINTS FRAMES.".into(),
            "STYLES DRAW THE DEVICE CASE, NOT THE GAME.".into(),
            "THE INSTALLER ONLY PACKAGES THE APP.".into(),
        ],
        review: Review::Fresh,
    }
}

/// The repository's own Oracle cartridge with a journal, so the menu, the
/// Codex, and the debriefs draw their fullest compositions.
fn oracle_cartridge(questions: Vec<QuizQuestion>) -> CartridgeSpec {
    let config = CodeQuestConfig::parse(include_str!("../../../CODEQUEST.toml"))
        .expect("the repository Oracle cartridge should parse");
    let lessons = (0..4)
        .map(|index| Lesson {
            question: format!("WHO OWNS THE GAME LOOP IN PART {index}?"),
            answer: "THE HEADLESS BEVY ENGINE".into(),
            rationale: "THE SHELL ONLY DRAWS FRAMES AND FORWARDS BUTTON EDGES.".into(),
            concept: Some(Concept::ALL[index % Concept::ALL.len()]),
            outstanding: index % 2 == 1,
            misconception: None,
            peeked: false,
            spaced_check: false,
        })
        .collect();
    let mastery = Mastery::from([
        (
            Concept::Purpose,
            LensRecord {
                first_try: 1,
                ..LensRecord::default()
            },
        ),
        (
            Concept::Invariant,
            LensRecord {
                first_try: 5,
                missed: 1,
                ..LensRecord::default()
            },
        ),
    ]);
    CartridgeSpec {
        id: CARTRIDGE_ID.into(),
        title: config.game.title.clone().unwrap_or_else(|| "BENCH".into()),
        mode: CartridgeMode::Quiz,
        provenance: RepositoryProvenance {
            authors: vec!["ADA LOVELACE".into(), "GRACE HOPPER".into()],
            first_year: Some(2020),
            latest_year: Some(2024),
            copyright: Some("Copyright (c) 2020-2024 Ada Lovelace".into()),
        },
        machine: Box::new(
            config
                .runtime_machine()
                .expect("the Oracle scene graph should compile")
                .expect("schema v2 should produce a runtime machine"),
        ),
        codequest: Some(Box::new(config)),
        quests: vec![],
        questions,
        question_batch_ends: Vec::new(),
        question_batch_levels: Vec::new(),
        lessons,
        mastery,
        question_attempts: Default::default(),
    }
}

fn powered(questions: Vec<QuizQuestion>) -> GameEngine {
    let mut engine = GameEngine::new();
    issue(
        &mut engine,
        EngineCommand::AiProvider(Some("claude".into())),
    );
    issue(
        &mut engine,
        EngineCommand::Cartridge(Some(oracle_cartridge(questions))),
    );
    issue(&mut engine, EngineCommand::Power(true));
    issue(&mut engine, EngineCommand::BootComplete);
    engine
}

/// Presses Start, letting each scene's skip hold elapse, until `screen`.
fn advance_to(engine: &mut GameEngine, screen: Screen) {
    for _ in 0..40 {
        if engine.screen() == screen {
            return;
        }
        press(engine, Button::Start);
        for _ in 0..30 {
            if engine.screen() == screen {
                return;
            }
            engine.update();
            let _ = engine.take_effects();
        }
    }
    panic!("never reached {screen:?}; stuck on {:?}", engine.screen());
}

fn batch() -> Vec<QuizQuestion> {
    (0..QUESTION_BATCH_SIZE).map(question).collect()
}

fn trial() -> GameEngine {
    let mut engine = powered(batch());
    advance_to(&mut engine, Screen::Quiz);
    engine
}

/// Focuses the answer (or the slot after it) and presses A.
fn commit(engine: &mut GameEngine, correct: bool) {
    let (answer, choices) = {
        let state = state(engine);
        let run = state.quiz.as_ref().unwrap();
        let question = &state.cartridge.as_ref().unwrap().questions[run.question];
        let order = run.display_order(question.choices.len());
        let answer = order
            .iter()
            .position(|source| *source == question.answer)
            .unwrap();
        (answer, question.choices.len())
    };
    let target = if correct {
        answer
    } else {
        (answer + 1) % choices
    };
    while state(engine).quiz.as_ref().unwrap().selected != target {
        press(engine, Button::Down);
    }
    press(engine, Button::A);
}

fn answer_until(engine: &mut GameEngine, correct: bool, screen: Screen) {
    for _ in 0..40 {
        if engine.screen() == screen {
            return;
        }
        commit(engine, correct);
        for _ in 0..QUIZ_FEEDBACK_TICKS {
            engine.update();
            let _ = engine.take_effects();
        }
        press(engine, Button::A);
    }
    panic!("never reached {screen:?}; stuck on {:?}", engine.screen());
}

struct Scene {
    name: &'static str,
    screen: Screen,
    setup: fn() -> GameEngine,
    /// A button held for the whole measurement, such as Datafall movement.
    hold: Option<Button>,
}

fn scenes() -> Vec<Scene> {
    vec![
        Scene {
            name: "chronicle",
            screen: Screen::Copyright,
            setup: || powered(batch()),
            hold: None,
        },
        Scene {
            name: "opening beats",
            screen: Screen::OpeningFanfare,
            setup: || {
                let mut engine = powered(batch());
                advance_to(&mut engine, Screen::OpeningFanfare);
                engine
            },
            hold: None,
        },
        Scene {
            name: "title",
            screen: Screen::Title,
            setup: || {
                let mut engine = powered(batch());
                advance_to(&mut engine, Screen::Title);
                engine
            },
            hold: None,
        },
        Scene {
            name: "menu",
            screen: Screen::QuizMenu,
            setup: || {
                let mut engine = powered(batch());
                advance_to(&mut engine, Screen::QuizMenu);
                engine
            },
            hold: None,
        },
        Scene {
            name: "atelier",
            screen: Screen::CharacterCreation,
            setup: || {
                let mut engine = powered(batch());
                advance_to(&mut engine, Screen::QuizMenu);
                press(&mut engine, Button::A);
                engine
            },
            hold: None,
        },
        Scene {
            name: "datafall",
            screen: Screen::Oracle,
            setup: || {
                let mut engine = powered(Vec::new());
                advance_to(&mut engine, Screen::Oracle);
                engine
            },
            hold: None,
        },
        Scene {
            name: "datafall moving",
            screen: Screen::Oracle,
            setup: || {
                let mut engine = powered(Vec::new());
                advance_to(&mut engine, Screen::Oracle);
                engine
            },
            hold: Some(Button::Left),
        },
        Scene {
            name: "trial",
            screen: Screen::Quiz,
            setup: trial,
            hold: None,
        },
        Scene {
            name: "lesson card",
            screen: Screen::Quiz,
            setup: || {
                let mut engine = trial();
                commit(&mut engine, false);
                engine
            },
            hold: None,
        },
        Scene {
            name: "codex mastery",
            screen: Screen::Codex,
            setup: || {
                let mut engine = powered(batch());
                advance_to(&mut engine, Screen::QuizMenu);
                press(&mut engine, Button::Down);
                press(&mut engine, Button::A);
                engine
            },
            hold: None,
        },
        Scene {
            name: "codex lesson",
            screen: Screen::Codex,
            setup: || {
                let mut engine = powered(batch());
                advance_to(&mut engine, Screen::QuizMenu);
                press(&mut engine, Button::Down);
                press(&mut engine, Button::A);
                press(&mut engine, Button::Right);
                engine
            },
            hold: None,
        },
        Scene {
            name: "ascension",
            screen: Screen::LevelUp,
            setup: || {
                let mut engine = trial();
                answer_until(&mut engine, true, Screen::LevelUp);
                engine
            },
            hold: None,
        },
        Scene {
            name: "aftermath",
            screen: Screen::GameOver,
            setup: || {
                let mut engine = trial();
                answer_until(&mut engine, false, Screen::GameOver);
                engine
            },
            hold: None,
        },
    ]
}

/// One runtime-loop tick without the thread and its sleep: the engine
/// update, then the frame copy, the note queue, and the transcript
/// publication the engine thread performs.
struct Runtime {
    frame: Vec<u8>,
    audio: AudioQueue,
    transcript: TranscriptChannel,
}

impl Runtime {
    fn new() -> Self {
        Self {
            frame: vec![0; FRAME_BYTES],
            audio: AudioQueue::default(),
            transcript: TranscriptChannel::default(),
        }
    }

    /// Returns the update time and the publication time.
    fn tick(&mut self, engine: &mut GameEngine) -> (Duration, Duration) {
        let start = Instant::now();
        engine.update();
        let _ = engine.take_effects();
        let updated = Instant::now();
        self.frame.copy_from_slice(engine.frame());
        self.audio.append(engine.take_audio());
        let (screen, sentences) = engine.transcript_sentences();
        self.transcript.publish(screen, sentences);
        (updated - start, updated.elapsed())
    }
}

/// FNV-1a over everything a tick publishes, so two builds can prove they
/// produce identical frames and transcripts for the same inputs.
struct Digest(u64);

impl Digest {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

struct Sample {
    total: Duration,
    update: Duration,
    publish: Duration,
    render: Duration,
    allocations: u64,
}

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index]
}

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e6
}

fn mean(durations: impl Iterator<Item = Duration>, count: usize) -> Duration {
    durations.sum::<Duration>() / count as u32
}

fn measure(scene: &Scene, digest: &mut Digest) -> Vec<Sample> {
    let mut samples = Vec::with_capacity(SAMPLES_PER_SCENE);
    let mut spare = Framebuffer::default();
    while samples.len() < SAMPLES_PER_SCENE {
        let mut engine = (scene.setup)();
        assert_eq!(engine.screen(), scene.screen, "{} setup", scene.name);
        if let Some(button) = scene.hold {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
        }
        let mut runtime = Runtime::new();
        // Warm the transcript channel and the director so the first sample
        // measures a steady tick, not the first publication of the screen.
        runtime.tick(&mut engine);
        while samples.len() < SAMPLES_PER_SCENE && engine.screen() == scene.screen {
            let before = allocations();
            let (update, publish) = runtime.tick(&mut engine);
            let allocations = allocations() - before;
            let _ = runtime.audio.drain();
            let render_start = Instant::now();
            draw_screen(&mut spare, state(&engine));
            let render = render_start.elapsed();
            assert_eq!(spare.pixels, runtime.frame, "{} redraw", scene.name);
            digest.write(&runtime.frame);
            if let Some(update) = runtime.transcript.since(0) {
                digest.write(update.text.as_bytes());
            }
            samples.push(Sample {
                total: update + publish,
                update,
                publish,
                render,
                allocations,
            });
        }
    }
    samples
}

/// The per-pixel plate blit the plate cache replaced, kept as the reference.
fn reference_blit_rgb_graded(rgb: &[u8; NATIVE_RGB_BYTES], grade: Option<[u16; 3]>) -> Vec<u8> {
    let mut pixels = vec![0; FRAME_BYTES];
    for (source, destination) in rgb
        .as_chunks::<3>()
        .0
        .iter()
        .zip(pixels.as_chunks_mut::<4>().0.iter_mut())
    {
        let [red, green, blue] = [source[0] as u16, source[1] as u16, source[2] as u16];
        let Some([base, cyan, gold]) = grade else {
            destination.copy_from_slice(&[source[0], source[1], source[2], 255]);
            continue;
        };
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
    pixels
}

/// The per-pixel awakening blit the class tables replaced.
fn reference_blit_awakening(ticks: u64) -> Vec<u8> {
    let mut pixels = vec![0; FRAME_BYTES];
    let cyan_strength = 38 + ticks.saturating_sub(36).min(108) as u16 * 190 / 108;
    let gold_strength = 38 + ticks.saturating_sub(112).min(108) as u16 * 197 / 108;
    let center_strength = 38 + ticks.saturating_sub(188).min(48) as u16 * 217 / 48;
    let ambient_strength = 34 + ticks.min(236) as u16 * 38 / 236;
    for (index, (source, destination)) in ORACLE_AWAKENING
        .as_chunks::<3>()
        .0
        .iter()
        .zip(pixels.as_chunks_mut::<4>().0.iter_mut())
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
    pixels
}

#[test]
fn cached_plates_draw_the_same_pixels_as_the_per_pixel_blit() {
    let plates = [
        ORACLE_AWAKENING,
        ORACLE_AWAKENING_SOURCE,
        ORACLE_AWAKENING_SIGNAL,
        ORACLE_AWAKENING_ARCHIVE,
        ORACLE_AWAKENING_CONVERGENCE,
        ORACLE_CHRONICLE,
        ORACLE_GATEWAY,
        ORACLE_ATELIER,
        ORACLE_SANCTUM,
        ORACLE_TRIAL,
        ORACLE_ASCENSION,
        ORACLE_AFTERMATH,
    ];
    // Every grade a renderer uses, plus one that scales every class apart.
    let grades = [
        None,
        Some([92, 105, 74]),
        Some([218, 238, 172]),
        Some([236, 250, 226]),
        Some([226, 244, 188]),
        Some([240, 250, 230]),
        Some([10, 128, 255]),
    ];
    let mut frame = Framebuffer::default();
    for plate in plates {
        for grade in grades {
            let expected = reference_blit_rgb_graded(plate, grade);
            // The first draw builds the cache entry and the second reads it;
            // a dirty frame in between proves the copy covers every byte.
            for _ in 0..2 {
                match grade {
                    None => frame.blit_rgb(plate),
                    Some([base, cyan, gold]) => frame.blit_rgb_graded(plate, base, cyan, gold),
                }
                assert!(frame.pixels == expected, "plate {grade:?} differs");
                frame.clear(MAGENTA);
            }
        }
    }
}

#[test]
fn the_awakening_tables_draw_the_same_pixels_as_the_per_pixel_blit() {
    let mut frame = Framebuffer::default();
    // Every strength saturates by tick 236, so this covers every distinct
    // frame, including the Oracle beat's 188-tick head start.
    for ticks in (0..=240).chain([u64::MAX / 2, u64::MAX]) {
        frame.blit_awakening(ticks);
        assert!(
            frame.pixels == reference_blit_awakening(ticks),
            "awakening tick {ticks} differs"
        );
    }
}

/// Mean time of `draw` over many calls on one framebuffer.
fn time_primitive(draw: impl Fn(&mut Framebuffer)) -> Duration {
    const CALLS: u32 = 400;
    let mut frame = Framebuffer::default();
    draw(&mut frame);
    let start = Instant::now();
    for _ in 0..CALLS {
        draw(std::hint::black_box(&mut frame));
    }
    start.elapsed() / CALLS
}

/// The drawing primitives the scenes are built from, timed alone.
#[test]
#[ignore = "benchmark"]
fn render_primitive_benchmark() {
    const LINE: &str = "WHICH LAYER DECIDES WHAT HAPPENS";
    type Primitive = (&'static str, fn(&mut Framebuffer));
    let primitives: [Primitive; 8] = [
        ("clear", |frame| frame.clear(NAVY)),
        ("blit_rgb", |frame| frame.blit_rgb(ORACLE_TRIAL)),
        ("blit_rgb_graded", |frame| {
            frame.blit_rgb_graded(ORACLE_TRIAL, 226, 244, 188)
        }),
        ("blit_awakening", |frame| frame.blit_awakening(120)),
        ("blit_rgba hero", |frame| {
            frame.blit_rgba(
                ORACLE_HEROES[0],
                HERO_SPRITE_WIDTH,
                HERO_SPRITE_HEIGHT,
                100,
                100,
                1,
            )
        }),
        ("rect 200x20", |frame| frame.rect(20, 60, 200, 20, VOID)),
        ("text 32 chars", |frame| frame.text(8, 40, LINE, PARCH, 1)),
        ("text 32 x2", |frame| frame.text(8, 40, LINE, PARCH, 2)),
    ];
    for (name, draw) in primitives {
        println!("{name:<16} {:>9.2} us", micros(time_primitive(draw)));
    }
}

#[test]
#[ignore = "benchmark"]
fn engine_tick_benchmark() {
    if cfg!(debug_assertions) {
        println!("note: debug build; run with --release for representative numbers");
    }
    println!(
        "{:<16} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "scene", "mean us", "p50 us", "p99 us", "max us", "update", "render", "publish"
    );
    let mut worst_p99 = Duration::ZERO;
    let mut digest = Digest::new();
    for scene in scenes() {
        let samples = measure(&scene, &mut digest);
        let count = samples.len();
        let mut totals: Vec<Duration> = samples.iter().map(|sample| sample.total).collect();
        totals.sort();
        let p99 = percentile(&totals, 0.99);
        worst_p99 = worst_p99.max(p99);
        let allocations = samples.iter().map(|sample| sample.allocations).sum::<u64>();
        println!(
            "{:<16} {:>9.1} {:>9.1} {:>9.1} {:>9.1} {:>9.1} {:>9.1} {:>9.1}   allocs/tick {:.1}",
            scene.name,
            micros(mean(totals.iter().copied(), count)),
            micros(percentile(&totals, 0.5)),
            micros(p99),
            micros(*totals.last().unwrap()),
            micros(mean(samples.iter().map(|sample| sample.update), count)),
            micros(mean(samples.iter().map(|sample| sample.render), count)),
            micros(mean(samples.iter().map(|sample| sample.publish), count)),
            allocations as f64 / count as f64,
        );
    }
    println!("worst p99: {:.1} us", micros(worst_p99));
    println!("output digest: {:016x}", digest.0);
}
