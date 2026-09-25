use super::*;
pub(super) use crate::learning::MASTERY_THRESHOLDS;
pub(super) use crate::scene_machine::{
    SceneHandler, SceneMachineDefinition, SceneMachineTemplate, SceneSignal, SceneSpec,
    SceneTransition,
};

pub(super) fn quiz_cartridge() -> CartridgeSpec {
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
        question_batch_levels: Vec::new(),
        lessons: Vec::new(),
        mastery: Mastery::new(),
        question_attempts: HashMap::new(),
    }
}

pub(super) fn oracle_template_cartridge() -> CartridgeSpec {
    let mut cartridge = quiz_cartridge();
    let config = CodeQuestConfig::parse(include_str!("../../../CODEQUEST.toml"))
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

pub(super) fn maybe_write_preview(name: &str, frame: &[u8]) {
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
    std::fs::write(directory.join(format!("{name}.ppm")), ppm).expect("preview should be writable");
}

pub(super) fn issue(engine: &mut GameEngine, command: EngineCommand) {
    engine.command(command);
    engine.update();
}

pub(super) fn finish_opening(engine: &mut GameEngine) {
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

pub(super) fn waiting_oracle_engine() -> GameEngine {
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

pub(super) fn playing_quiz_engine() -> GameEngine {
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

pub(super) fn press(engine: &mut GameEngine, button: Button) {
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

pub(super) fn engine_state(engine: &GameEngine) -> &GameState {
    engine.app.world().resource::<GameState>()
}

/// Answers the engine's newest question request with `questions`.
pub(super) fn deliver(engine: &mut GameEngine, cartridge_id: &str, questions: Vec<QuizQuestion>) {
    let seq = engine_state(engine).question_request_seq;
    issue(
        engine,
        EngineCommand::Questions {
            cartridge_id: cartridge_id.into(),
            result: Ok(questions),
            seq,
        },
    );
}

/// Fails the engine's newest question request with `reason`.
pub(super) fn fail(engine: &mut GameEngine, cartridge_id: &str, reason: &str) {
    let seq = engine_state(engine).question_request_seq;
    issue(
        engine,
        EngineCommand::Questions {
            cartridge_id: cartridge_id.into(),
            result: Err(reason.into()),
            seq,
        },
    );
}

pub(super) fn current_question(engine: &GameEngine) -> QuizQuestion {
    let state = engine_state(engine);
    let run = state.quiz.as_ref().unwrap();
    state.cartridge.as_ref().unwrap().questions[run.question].clone()
}

/// The display slot that currently shows the correct answer.
pub(super) fn answer_slot(engine: &GameEngine) -> usize {
    let question = current_question(engine);
    let run = engine_state(engine).quiz.as_ref().unwrap();
    run.display_order(question.choices.len())
        .iter()
        .position(|source| *source == question.answer)
        .unwrap()
}

/// Moves focus to the answer slot, or to the slot after it for a miss.
pub(super) fn focus_choice(engine: &mut GameEngine, correct: bool) {
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

pub(super) fn commit(engine: &mut GameEngine, correct: bool) {
    focus_choice(engine, correct);
    press(engine, Button::A);
}

/// Waits out the lesson hold and continues past the lesson card.
pub(super) fn finish_lesson(engine: &mut GameEngine) {
    for _ in 0..QUIZ_FEEDBACK_TICKS {
        engine.update();
    }
    press(engine, Button::A);
}

pub(super) fn concept_question(index: usize) -> QuizQuestion {
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
        review: Review::Fresh,
    }
}

/// A quiz engine playing one batch of distinct concept questions.
pub(super) fn batch_quiz_engine(count: usize) -> GameEngine {
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
    deliver(
        &mut engine,
        "/tmp/engine-test",
        (0..count).map(concept_question).collect(),
    );
    for _ in 0..75 {
        engine.update();
    }
    assert_eq!(engine.screen(), Screen::Quiz);
    engine
}

pub(super) fn color_pixels_in_region(
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

pub(super) fn total_luminance(frame: &[u8]) -> u64 {
    frame
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| pixel[0] as u64 + pixel[1] as u64 + pixel[2] as u64)
        .sum()
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LayoutBounds {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

pub(super) fn text_bounds(x: i32, y: i32, text: &str, scale: i32) -> LayoutBounds {
    LayoutBounds {
        x,
        y,
        width: text_width(text, scale),
        height: 7 * scale,
    }
}

pub(super) fn compact_text_bounds(x: i32, y: i32, text: &str) -> LayoutBounds {
    LayoutBounds {
        x,
        y,
        width: text.chars().count() as i32 * GLYPH_WIDTH,
        height: 7,
    }
}

pub(super) fn alpha_bounds(rgba: &[u8], width: usize, height: usize) -> LayoutBounds {
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

pub(super) fn centered_text_bounds(y: i32, text: &str, scale: i32) -> LayoutBounds {
    let width = text_width(text, scale);
    text_bounds((WIDTH as i32 - width) / 2, y, text, scale)
}

pub(super) fn centered_text_in_bounds(
    container: LayoutBounds,
    y: i32,
    text: &str,
    scale: i32,
) -> LayoutBounds {
    let width = text_width(text, scale);
    text_bounds(container.x + (container.width - width) / 2, y, text, scale)
}

pub(super) fn ui_box_bounds(bounds: UiBox) -> LayoutBounds {
    LayoutBounds {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    }
}

pub(super) fn centered_text_box_bounds(bounds: UiBox, text: &str, scale: i32) -> LayoutBounds {
    let width = text_width(text, scale);
    text_bounds(
        bounds.x + (bounds.width - width) / 2,
        bounds.y + (bounds.height - 7 * scale) / 2,
        text,
        scale,
    )
}

pub(super) fn centered_compact_text_box_bounds(bounds: UiBox, text: &str) -> LayoutBounds {
    let width = text.chars().count() as i32 * GLYPH_WIDTH;
    compact_text_bounds(
        bounds.x + (bounds.width - width) / 2,
        bounds.y + (bounds.height - 7) / 2,
        text,
    )
}

pub(super) fn bounds_contains(container: LayoutBounds, child: LayoutBounds) -> bool {
    child.x >= container.x
        && child.y >= container.y
        && child.x + child.width <= container.x + container.width
        && child.y + child.height <= container.y + container.height
}

pub(super) fn bounds_are_disjoint(left: LayoutBounds, right: LayoutBounds) -> bool {
    left.x + left.width <= right.x
        || right.x + right.width <= left.x
        || left.y + left.height <= right.y
        || right.y + right.height <= left.y
}

pub(super) fn horizontal_centers_align(container: LayoutBounds, child: LayoutBounds) -> bool {
    ((container.x * 2 + container.width) - (child.x * 2 + child.width)).abs() <= 1
}

pub(super) fn vertical_centers_align(container: LayoutBounds, child: LayoutBounds) -> bool {
    ((container.y * 2 + container.height) - (child.y * 2 + child.height)).abs() <= 1
}

pub(super) fn relative_luminance(color: Color) -> f64 {
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

pub(super) fn contrast_ratio(foreground: Color, background: Color) -> f64 {
    let foreground = relative_luminance(foreground);
    let background = relative_luminance(background);
    let (lighter, darker) = if foreground > background {
        (foreground, background)
    } else {
        (background, foreground)
    };
    (lighter + 0.05) / (darker + 0.05)
}

/// A result-screen state whose every debrief row carries its widest copy:
/// an unlit insight, the widest tier, two-digit counts, and a lens woken to
/// its third rune.
pub(super) fn worst_case_debrief_state() -> GameState {
    let mut cartridge = oracle_template_cartridge();
    cartridge.lessons = (0..120)
        .map(|index| Lesson {
            question: format!("MISSED {index}"),
            outstanding: true,
            ..Lesson::default()
        })
        .collect();
    cartridge.mastery = Mastery::from([(
        Concept::Invariant,
        LensRecord {
            first_try: 5,
            ..LensRecord::default()
        },
    )]);
    GameState {
        cartridge: Some(cartridge),
        quiz: Some(QuizRun {
            score: 0,
            level: 99,
            ledger: RunLedger {
                first_try: 150,
                first_try_right: 120,
                redeemed: 120,
                last_batch: (120, 150),
                ..RunLedger::default()
            },
            ..QuizRun::new()
        }),
        ..Default::default()
    }
}

/// Asserts `frame` shows exactly `text` in `color` at `bounds` over the
/// pixels `background` holds there.
pub(super) fn assert_text_drawn(
    frame: &Framebuffer,
    background: &Framebuffer,
    bounds: LayoutBounds,
    text: &str,
    color: Color,
) {
    let mut expected = Framebuffer {
        pixels: background.pixels.clone(),
    };
    expected.text(bounds.x, bounds.y, text, color, 1);
    let x_range = bounds.x as usize..(bounds.x + bounds.width) as usize;
    let y_range = bounds.y as usize..(bounds.y + bounds.height) as usize;
    assert!(
        frame_region(&frame.pixels, x_range.clone(), y_range.clone())
            == frame_region(&expected.pixels, x_range, y_range),
        "`{text}` is not drawn at {bounds:?}"
    );
}

pub(super) fn frame_region(
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

pub(super) fn hero_pixels(engine: &GameEngine) -> Vec<u8> {
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

pub(super) fn channels(color: Color) -> (u8, u8, u8) {
    (color.0, color.1, color.2)
}

pub(super) fn trial_counter(engine: &GameEngine) -> (String, (u8, u8, u8)) {
    let (text, color) = question_counter(engine_state(engine), ("TRIAL ", "RETRY "), CYAN);
    (text, channels(color))
}

pub(super) fn state_mut(engine: &mut GameEngine) -> Mut<'_, GameState> {
    engine.app.world_mut().resource_mut::<GameState>()
}

pub(super) fn retry_note(engine: &GameEngine) -> Option<RetryNote> {
    engine_state(engine).quiz.as_ref().unwrap().retry_note
}

pub(super) fn lesson_question() -> QuizQuestion {
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
        review: Review::Fresh,
    }
}

/// 31-character choices and rationales that fill all three 34-column rows.
pub(super) fn worst_case_lesson_question() -> QuizQuestion {
    let row = format!("{} {}", "W".repeat(16), "W".repeat(17));
    QuizQuestion {
        question: "Q".repeat(QUIZ_QUESTION_COLUMNS),
        choices: ["A", "B", "C", "D"]
            .map(|letter| letter.repeat(QUIZ_CHOICE_CHARS))
            .to_vec(),
        answer: 0,
        concept: Some(Concept::Invariant),
        rationales: vec![[row.as_str(), row.as_str(), row.as_str()].join(" "); 4],
        review: Review::Fresh,
    }
}

pub(super) fn lesson_state(question: QuizQuestion, correct: bool, live: bool) -> GameState {
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
            retry_note: (!correct).then_some(RetryNote::In(RETRY_GAP)),
            ..QuizRun::new()
        }),
        ..Default::default()
    }
}

pub(super) fn request_seqs(effects: &[EngineEffect]) -> Vec<(u32, u64)> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            EngineEffect::RequestQuestions { level, seq, .. } => Some((*level, *seq)),
            _ => None,
        })
        .collect()
}

pub(super) fn scene_spec(
    id: &str,
    handler: SceneHandler,
    transitions: &[(SceneSignal, &str, Option<u64>)],
) -> SceneSpec {
    SceneSpec {
        id: id.into(),
        handler,
        transitions: transitions
            .iter()
            .map(|(signal, target, after_ticks)| SceneTransition {
                signal: *signal,
                target: (*target).into(),
                after_ticks: *after_ticks,
            })
            .collect(),
    }
}

pub(super) fn oracle_line_texts(state: &GameState) -> Vec<String> {
    state
        .oracle_line()
        .map(|line| {
            oracle_line_segments(&line, SANCTUM_ORACLE_LINE_BOX.width)
                .into_iter()
                .map(|(text, _)| text)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn recall_lesson(answer: &str, concept: Option<Concept>, outstanding: bool) -> Lesson {
    Lesson {
        question: format!("WHAT IS {answer}?"),
        answer: answer.into(),
        rationale: String::new(),
        concept,
        outstanding,
        misconception: None,
        peeked: false,
        spaced_check: false,
    }
}

/// A waiting Oracle state whose journal holds `lessons`.
pub(super) fn waiting_state_with(lessons: Vec<Lesson>) -> GameState {
    let mut cartridge = oracle_template_cartridge();
    cartridge.questions.clear();
    cartridge.lessons = lessons;
    GameState {
        cartridge: Some(cartridge),
        ai_provider: Some("CLAUDE".into()),
        quiz: Some(QuizRun::new()),
        questions_loading: true,
        ..Default::default()
    }
}

pub(super) fn recalled_answers(state: &GameState) -> Vec<String> {
    state
        .recall_order()
        .into_iter()
        .map(|lesson| lesson.answer.clone())
        .collect()
}

/// True while a failed request waits for the tick that sends its retry.
pub(super) fn cycle_start(engine: &GameEngine) -> bool {
    let state = engine_state(engine);
    !state.questions_loading && state.question_failure.is_some()
}

pub(super) fn last_cue(engine: &GameEngine) -> Option<audio::Cue> {
    engine.app.world().resource::<AudioOut>().last_cue()
}

/// Presses and releases `button`, returning the cue each edge started.
pub(super) fn tap_cues(engine: &mut GameEngine, button: Button) -> [Option<audio::Cue>; 2] {
    [true, false].map(|pressed| {
        issue(engine, EngineCommand::Input { button, pressed });
        last_cue(engine)
    })
}

pub(super) const WORST_LESSON_QUESTION: &str =
    "WHICH GUARANTEE KEEPS THE ENGINE AND THE DEVICE SHELL FROM EVER DISAGREEING ABOUT THE ACTIVE SCENE?";
pub(super) const WORST_LESSON_RATIONALE: &str =
    "THE SHELL FORWARDS INPUT EDGES AND DRAWS FRAMES, SO ONLY THE ENGINE CAN MOVE THE SCENE MACHINE.";
/// A full-width wrong pick and a three-row misconception: the sealed
/// page's worst case.
pub(super) const WORST_LESSON_PICK: &str = "THE SHELL PICKS EACH NEXT SCENE";
pub(super) const WORST_LESSON_MISCONCEPTION: &str =
    "THE SHELL ONLY FORWARDS EDGES AND DRAWS FRAMES, SO IT NEVER HOLDS THE SCENE MACHINE STATE.";

pub(super) fn journal_lessons() -> Vec<Lesson> {
    vec![
        Lesson {
            question: "WHO OWNS THE GAME LOOP?".into(),
            answer: "THE HEADLESS BEVY ENGINE".into(),
            rationale: "THE SHELL ONLY DRAWS FRAMES AND FORWARDS BUTTON EDGES.".into(),
            concept: Some(Concept::Responsibility),
            outstanding: false,
            misconception: Some((
                "THE WEB SHELL THAT DRAWS FRAMES".into(),
                "THE SHELL ONLY PAINTS WHAT THE ENGINE HANDS IT.".into(),
            )),
            peeked: false,
            spaced_check: false,
        },
        Lesson {
            question: WORST_LESSON_QUESTION.into(),
            answer: "ONLY THE ENGINE CHANGES SCENES".into(),
            rationale: WORST_LESSON_RATIONALE.into(),
            concept: Some(Concept::Invariant),
            outstanding: true,
            misconception: Some((WORST_LESSON_PICK.into(), WORST_LESSON_MISCONCEPTION.into())),
            peeked: false,
            spaced_check: false,
        },
        Lesson {
            question: "WHAT DID THIS OLDER SAVE ASK?".into(),
            answer: "A QUESTION WITHOUT A LENS".into(),
            rationale: String::new(),
            concept: None,
            outstanding: false,
            misconception: None,
            peeked: false,
            spaced_check: false,
        },
    ]
}

pub(super) fn journal_mastery() -> Mastery {
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
                ..LensRecord::default()
            },
        ),
        (
            Concept::Invariant,
            LensRecord {
                first_try: 4,
                redeemed: 1,
                missed: 3,
                ..LensRecord::default()
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

pub(super) fn journal_cartridge() -> CartridgeSpec {
    let mut cartridge = quiz_cartridge();
    cartridge.lessons = journal_lessons();
    cartridge.mastery = journal_mastery();
    cartridge
}

pub(super) fn quiz_menu_engine(cartridge: CartridgeSpec) -> GameEngine {
    let mut engine = GameEngine::new();
    issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
    issue(&mut engine, EngineCommand::Power(true));
    finish_opening(&mut engine);
    press(&mut engine, Button::Start);
    assert_eq!(engine.screen(), Screen::QuizMenu);
    engine
}

pub(super) fn game_state(engine: &GameEngine) -> &GameState {
    engine.app.world().resource::<GameState>()
}

/// Classifies the three 5x7 runes of a meter drawn at (`x`, `y`).
pub(super) fn meter_styles(frame: &[u8], x: i32, y: i32) -> [RuneStyle; 3] {
    std::array::from_fn(|index| {
        let rune_x = (x + index as i32 * 7) as usize;
        let xs = rune_x..rune_x + 5;
        let ys = y as usize..y as usize + 7;
        if color_pixels_in_region(frame, PARCH, xs.clone(), ys.clone()) > 0 {
            RuneStyle::Lit
        } else if color_pixels_in_region(frame, AMBER, xs.clone(), ys.clone()) > 0 {
            RuneStyle::Cracked
        } else {
            assert!(
                color_pixels_in_region(frame, ASH, xs, ys) > 0,
                "rune {index}"
            );
            RuneStyle::Unlit
        }
    })
}

/// A lens record with nine first-try successes whose newest five graded
/// outcomes hold `right` successes.
pub(super) fn gated_record(right: u32) -> LensRecord {
    LensRecord {
        first_try: 9,
        recent: ((1u32 << right) - 1) as u8,
        recent_len: learning::RECENT_CAPACITY,
        ..LensRecord::default()
    }
}

pub(super) fn brightest_plate_color(plate: &[u8; NATIVE_RGB_BYTES], bounds: LayoutBounds) -> Color {
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

pub(super) fn inset(bounds: UiBox) -> LayoutBounds {
    LayoutBounds {
        x: bounds.x + 1,
        y: bounds.y + 1,
        width: bounds.width - 2,
        height: bounds.height - 2,
    }
}
