use std::collections::{HashSet, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use font8x8::{UnicodeFonts, BASIC_FONTS};

pub const WIDTH: usize = 240;
pub const HEIGHT: usize = 160;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT * 4;
const FRAME_TIME: Duration = Duration::from_nanos(16_666_667);

const INK: Color = Color::rgb(26, 28, 44);
const NAVY: Color = Color::rgb(41, 54, 111);
const ROYAL: Color = Color::rgb(59, 93, 201);
const SKY: Color = Color::rgb(65, 166, 246);
const PARCH: Color = Color::rgb(244, 244, 244);
const MIST: Color = Color::rgb(148, 176, 194);
const GOLD: Color = Color::rgb(255, 205, 117);
const GREEN: Color = Color::rgb(56, 183, 100);
const RED: Color = Color::rgb(177, 62, 83);
const PLUM: Color = Color::rgb(93, 39, 93);
const CRAB: Color = Color::rgb(206, 142, 107);

#[derive(Clone, Copy)]
struct Color(u8, u8, u8);

impl Color {
    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(r, g, b)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CartridgeMode {
    Quiz,
    Custom,
}

#[derive(Clone, Debug)]
pub struct CartridgeSpec {
    pub id: String,
    pub title: String,
    pub mode: CartridgeMode,
    pub quests: Vec<QuestSpec>,
    pub questions: Vec<QuizQuestion>,
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
    Title,
    QuizMenu,
    Oracle,
    Quiz,
    GameOver,
    QuestSelect,
    Battle,
    Victory,
    Defeat,
}

#[derive(Clone, Debug)]
enum EngineCommand {
    Power(bool),
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
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[color.0, color.1, color.2, 255]);
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

    fn text(&mut self, x: i32, y: i32, text: &str, color: Color, scale: i32) {
        let mut cursor = x;
        for ch in text.to_ascii_uppercase().chars() {
            if let Some(glyph) = BASIC_FONTS.get(ch) {
                for (gy, row) in glyph.iter().enumerate() {
                    for gx in 0..8 {
                        if row & (1 << gx) != 0 {
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
            }
            cursor += 8 * scale;
        }
    }

    fn centered_text(&mut self, y: i32, text: &str, color: Color, scale: i32) {
        let width = text.chars().count() as i32 * 8 * scale;
        self.text((WIDTH as i32 - width) / 2, y, text, color, scale);
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
            self.text(x, y + line_no as i32 * 9, &line, color, 1);
        }
    }
}

#[derive(Clone, Debug)]
struct QuizRun {
    question: usize,
    selected: usize,
    hearts: u8,
    score: u32,
    feedback: Option<(bool, u16)>,
}

#[derive(Resource)]
struct GameState {
    powered: bool,
    cartridge: Option<CartridgeSpec>,
    screen: Screen,
    screen_ticks: u64,
    held: HashSet<Button>,
    menu_selected: usize,
    quest_selected: usize,
    quiz: Option<QuizRun>,
    pending_questions: Option<(String, Vec<QuizQuestion>)>,
    oracle_jump: u16,
    logs: VecDeque<(String, bool)>,
    active_boss: String,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            powered: false,
            cartridge: None,
            screen: Screen::Off,
            screen_ticks: 0,
            held: HashSet::new(),
            menu_selected: 0,
            quest_selected: 0,
            quiz: None,
            pending_questions: None,
            oracle_jump: 0,
            logs: VecDeque::new(),
            active_boss: String::new(),
        }
    }
}

impl GameState {
    fn transition(&mut self, screen: Screen) {
        self.screen = screen;
        self.screen_ticks = 0;
    }

    fn has_game(&self) -> bool {
        self.cartridge.is_some()
    }

    fn cartridge_mode(&self) -> Option<CartridgeMode> {
        self.cartridge.as_ref().map(|cart| cart.mode)
    }

    fn question_count(&self) -> usize {
        self.cartridge
            .as_ref()
            .map_or(0, |cart| cart.questions.len())
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
    pub fn spawn() -> Self {
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
                        handle_effect(effect, &engine_sender, &running_child);
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

    pub fn set_cartridge(&self, cartridge: Option<CartridgeSpec>) -> Result<(), String> {
        self.send(EngineCommand::Cartridge(cartridge))
    }

    pub fn input(&self, button: Button, pressed: bool) -> Result<(), String> {
        self.send(EngineCommand::Input { button, pressed })
    }

    pub fn replace_questions(
        &self,
        cartridge_id: String,
        questions: Vec<QuizQuestion>,
    ) -> Result<(), String> {
        self.send(EngineCommand::Questions {
            cartridge_id,
            questions,
        })
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
            EngineCommand::Cartridge(cartridge) => {
                state.cartridge = cartridge;
                state.quest_selected = 0;
                state.menu_selected = 0;
                state.quiz = None;
                state.pending_questions = None;
                if state.powered {
                    state.transition(Screen::Boot);
                }
            }
            EngineCommand::Questions {
                cartridge_id,
                questions,
            } => {
                if state.screen == Screen::Quiz {
                    state.pending_questions = Some((cartridge_id, questions));
                    continue;
                }
                if let Some(cartridge) = state.cartridge.as_mut() {
                    if cartridge.id == cartridge_id
                        && cartridge.mode == CartridgeMode::Quiz
                        && !questions.is_empty()
                    {
                        cartridge.questions = questions;
                    }
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
                    for wrapped in wrap_text(&line, 27).into_iter().take(3) {
                        state.logs.push_back((wrapped, stderr));
                    }
                    while state.logs.len() > 8 {
                        state.logs.pop_front();
                    }
                }
            }
            EngineCommand::QuestDone { success } => {
                if state.screen == Screen::Battle {
                    state.transition(if success {
                        Screen::Victory
                    } else {
                        Screen::Defeat
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
        Screen::Boot => {
            if state.has_game() && state.screen_ticks >= 10 {
                state.transition(Screen::Title);
            }
        }
        Screen::Title => {
            if matches!(button, Button::A | Button::Start) {
                match state.cartridge_mode() {
                    Some(CartridgeMode::Quiz) => state.transition(Screen::QuizMenu),
                    Some(CartridgeMode::Custom) => state.transition(Screen::QuestSelect),
                    None => {}
                }
            }
        }
        Screen::QuizMenu => match button {
            Button::Up | Button::Down => state.menu_selected = 1 - state.menu_selected,
            Button::B => state.transition(Screen::Title),
            Button::A | Button::Start => {
                if state.menu_selected == 1 {
                    state.transition(Screen::Title);
                } else {
                    state.quiz = Some(QuizRun {
                        question: 0,
                        selected: 0,
                        hearts: 3,
                        score: 0,
                        feedback: None,
                    });
                    state.transition(Screen::Oracle);
                }
            }
            _ => {}
        },
        Screen::Oracle => {
            if button == Button::A {
                state.oracle_jump = 25;
            } else if button == Button::B {
                state.transition(Screen::QuizMenu);
            }
        }
        Screen::Quiz => {
            let question_count = state.question_count();
            let Some(run) = state.quiz.as_mut() else {
                return;
            };
            if run.feedback.is_some() {
                return;
            }
            let choice_count = state
                .cartridge
                .as_ref()
                .and_then(|cart| cart.questions.get(run.question % question_count.max(1)))
                .map_or(1, |question| question.choices.len().max(1));
            match button {
                Button::Up => run.selected = (run.selected + choice_count - 1) % choice_count,
                Button::Down => run.selected = (run.selected + 1) % choice_count,
                Button::B => state.transition(Screen::QuizMenu),
                Button::A => {
                    let answer = state
                        .cartridge
                        .as_ref()
                        .and_then(|cart| cart.questions.get(run.question % question_count.max(1)))
                        .map_or(0, |question| question.answer);
                    let correct = run.selected == answer;
                    if correct {
                        run.score += 100;
                    } else {
                        run.hearts = run.hearts.saturating_sub(1);
                    }
                    run.feedback = Some((correct, 45));
                }
                _ => {}
            }
        }
        Screen::GameOver => {
            if matches!(button, Button::A | Button::B | Button::Start) {
                state.transition(Screen::QuizMenu);
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
                Button::B => state.transition(Screen::Title),
                Button::A | Button::Start if count > 0 => {
                    let quest =
                        state.cartridge.as_ref().unwrap().quests[state.quest_selected].clone();
                    state.active_boss = quest.boss;
                    state.logs.clear();
                    state.logs.push_back((format!("> {}", quest.name), false));
                    state.transition(Screen::Battle);
                    effects.0.push_back(EngineEffect::RunQuest(quest.command));
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
            Button::A | Button::B => state.transition(Screen::QuestSelect),
            Button::Start => state.transition(Screen::QuestSelect),
            _ => {}
        },
    }
}

fn advance_game(mut state: ResMut<GameState>) {
    state.screen_ticks = state.screen_ticks.saturating_add(1);
    if state.oracle_jump > 0 {
        state.oracle_jump -= 1;
    }
    if state.screen == Screen::Oracle {
        if let Some((cartridge_id, questions)) = state.pending_questions.take() {
            if let Some(cartridge) = state.cartridge.as_mut() {
                if cartridge.id == cartridge_id && !questions.is_empty() {
                    cartridge.questions = questions;
                }
            }
        }
    }
    match state.screen {
        Screen::Boot if state.has_game() && state.screen_ticks >= 90 => {
            state.transition(Screen::Title)
        }
        Screen::Oracle if state.screen_ticks >= 75 => {
            if state.question_count() == 0 {
                state.transition(Screen::GameOver);
            } else {
                state.transition(Screen::Quiz);
            }
        }
        Screen::Quiz => {
            let mut next_screen = None;
            if let Some(run) = state.quiz.as_mut() {
                if let Some((correct, ticks)) = run.feedback.as_mut() {
                    let _ = correct;
                    *ticks = ticks.saturating_sub(1);
                    if *ticks == 0 {
                        run.feedback = None;
                        if run.hearts == 0 {
                            next_screen = Some(Screen::GameOver);
                        } else {
                            run.question += 1;
                            run.selected = 0;
                            if run.question % 6 == 0 {
                                next_screen = Some(Screen::Oracle);
                            }
                        }
                    }
                }
            }
            if let Some(screen) = next_screen {
                state.transition(screen);
            }
        }
        _ => {}
    }
}

fn render(mut frame: ResMut<Framebuffer>, state: Res<GameState>) {
    match state.screen {
        Screen::Off => frame.clear(INK),
        Screen::Boot => render_boot(&mut frame, &state),
        Screen::Title => render_title(&mut frame, &state),
        Screen::QuizMenu => render_quiz_menu(&mut frame, &state),
        Screen::Oracle => render_oracle(&mut frame, &state),
        Screen::Quiz => render_quiz(&mut frame, &state),
        Screen::GameOver => render_game_over(&mut frame, &state),
        Screen::QuestSelect => render_quest_select(&mut frame, &state),
        Screen::Battle => render_battle(&mut frame, &state),
        Screen::Victory => render_result(&mut frame, true),
        Screen::Defeat => render_result(&mut frame, false),
    }
}

fn render_boot(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(PARCH);
    frame.centered_text(54, "CODEQUEST", NAVY, 2);
    frame.centered_text(73, "ADVANCE", ROYAL, 2);
    frame.rect(68, 96, 104, 2, MIST);
    if !state.has_game() && state.screen_ticks > 50 && (state.screen_ticks / 30).is_multiple_of(2) {
        frame.centered_text(115, "INSERT CARTRIDGE", RED, 1);
    }
}

fn render_title(frame: &mut Framebuffer, state: &GameState) {
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

fn render_oracle(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(INK);
    for index in 0..30 {
        let x = ((index * 71 + state.screen_ticks as usize * 2) % WIDTH) as i32;
        frame.pixel(x, 28 + (index * 29 % 82) as i32, MIST);
    }
    frame.centered_text(20, "ORACLE LOADING", SKY, 1);
    let phase = (state.screen_ticks % 60) / 15;
    frame.centered_text(34, &".".repeat(phase as usize + 1), GOLD, 1);
    frame.rect(0, 132, WIDTH as i32, 28, PLUM);
    frame.rect(0, 128, WIDTH as i32, 4, GREEN);
    let jump = if state.oracle_jump > 0 {
        let t = state.oracle_jump as i32 - 12;
        12 - (t.abs() / 2)
    } else {
        0
    };
    draw_crab(frame, 104, 111 - jump);
    frame.centered_text(148, "A:JUMP", PARCH, 1);
}

fn render_quiz(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(NAVY);
    let Some(run) = state.quiz.as_ref() else {
        return;
    };
    frame.rect(0, 0, WIDTH as i32, 16, INK);
    frame.text(5, 4, &format!("Q{:02}", run.question + 1), SKY, 1);
    frame.text(
        84,
        4,
        &format!("HP {}", "*".repeat(run.hearts as usize)),
        RED,
        1,
    );
    frame.text(176, 4, &format!("{:04}", run.score), GOLD, 1);
    let Some(cart) = state.cartridge.as_ref() else {
        return;
    };
    let Some(question) = cart
        .questions
        .get(run.question % cart.questions.len().max(1))
    else {
        return;
    };
    frame.rect(5, 23, 230, 42, INK);
    frame.outline(5, 23, 230, 42, SKY);
    frame.wrapped_text(11, 30, &question.question, PARCH, 27, 3);
    for (index, choice) in question.choices.iter().take(4).enumerate() {
        let y = 73 + index as i32 * 20;
        let mut color = PARCH;
        if run.selected == index {
            frame.rect(5, y - 3, 230, 16, ROYAL);
            frame.text(9, y, ">", GOLD, 1);
        }
        if let Some((correct, _)) = run.feedback {
            if index == question.answer {
                color = GREEN;
            } else if run.selected == index && !correct {
                color = RED;
            }
        }
        frame.text(21, y, &truncate(choice, 25), color, 1);
    }
    frame.text(5, 151, "A:ANSWER", MIST, 1);
    frame.text(163, 151, "B:BACK", MIST, 1);
}

fn render_game_over(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(INK);
    frame.centered_text(43, "GAME OVER", RED, 2);
    if let Some(run) = state.quiz.as_ref() {
        frame.centered_text(81, &format!("SCORE {:04}", run.score), GOLD, 1);
    } else {
        frame.centered_text(81, "NO QUESTIONS FOUND", GOLD, 1);
    }
    if (state.screen_ticks / 30).is_multiple_of(2) {
        frame.centered_text(124, "PRESS A", PARCH, 1);
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
        frame.text(20, y, &truncate(&quest.name, 25), PARCH, 1);
        frame.text(20, y + 9, &truncate(&quest.boss, 25), MIST, 1);
    }
    frame.centered_text(147, "A:FIGHT  B:BACK", MIST, 1);
}

fn render_battle(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 69, INK);
    frame.text(6, 5, &truncate(&state.active_boss, 27), RED, 1);
    draw_crab(frame, 34, 45);
    draw_boss(frame, 183, 29, state.screen_ticks);
    frame.rect(0, 68, WIDTH as i32, 2, SKY);
    frame.outline(4, 75, 232, 68, MIST);
    for (index, (line, stderr)) in state.logs.iter().rev().take(7).rev().enumerate() {
        frame.text(
            9,
            80 + index as i32 * 9,
            &truncate(line, 27),
            if *stderr { RED } else { PARCH },
            1,
        );
    }
    frame.text(5, 150, "B:ABORT", GOLD, 1);
    frame.text(143, 150, "RUST PROCESS", GREEN, 1);
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

fn draw_crab(frame: &mut Framebuffer, x: i32, y: i32) {
    frame.rect(x + 4, y, 20, 10, CRAB);
    frame.rect(x, y + 5, 28, 8, CRAB);
    frame.rect(x + 3, y + 13, 5, 4, CRAB);
    frame.rect(x + 20, y + 13, 5, 4, CRAB);
    frame.rect(x + 7, y + 3, 3, 3, INK);
    frame.rect(x + 18, y + 3, 3, 3, INK);
}

fn draw_boss(frame: &mut Framebuffer, x: i32, y: i32, tick: u64) {
    let bob = ((tick / 15) % 2) as i32;
    frame.rect(x, y + bob, 30, 27, PLUM);
    frame.rect(x - 4, y + 7 + bob, 38, 13, PLUM);
    frame.rect(x + 5, y + 7 + bob, 5, 5, GOLD);
    frame.rect(x + 20, y + 7 + bob, 5, 5, GOLD);
    frame.rect(x + 8, y + 20 + bob, 14, 3, RED);
}

fn handle_effect(
    effect: EngineEffect,
    sender: &mpsc::Sender<EngineCommand>,
    child: &Arc<Mutex<Option<Child>>>,
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
    let mut child = match Command::new("bash")
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
        if candidate.chars().count() <= 13 {
            *lines.last_mut().unwrap() = candidate;
        } else if lines.len() == 1 {
            lines.push(truncate(word, 13));
        }
    }
    if lines[0].is_empty() {
        lines[0] = truncate(title, 13);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiz_cartridge() -> CartridgeSpec {
        CartridgeSpec {
            id: "/tmp/engine-test".into(),
            title: "ENGINE TEST".into(),
            mode: CartridgeMode::Quiz,
            quests: vec![],
            questions: vec![QuizQuestion {
                question: "WHO OWNS THE GAME LOOP?".into(),
                choices: vec!["BEVY".into(), "CSS".into(), "WEBKIT".into(), "HTML".into()],
                answer: 0,
            }],
        }
    }

    fn issue(engine: &mut GameEngine, command: EngineCommand) {
        engine.command(command);
        engine.update();
    }

    #[test]
    fn framebuffer_is_always_fixed_resolution() {
        let mut engine = GameEngine::new();
        assert_eq!(engine.frame().len(), FRAME_BYTES);
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
    fn jumping_on_oracle_screen_cannot_change_resolution() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        for _ in 0..90 {
            engine.update();
        }
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
        assert_eq!(engine.screen(), Screen::Oracle);
        for _ in 0..30 {
            engine.update();
            assert_eq!(engine.frame().len(), FRAME_BYTES);
        }
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
        assert_eq!(
            state.cartridge.as_ref().unwrap().questions[0].question,
            "NEW BATCH"
        );
    }

    #[test]
    fn bevy_owns_boot_and_navigation_state() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        assert_eq!(engine.screen(), Screen::Boot);
        for _ in 0..90 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Title);
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
    fn held_button_only_generates_one_edge() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        for _ in 0..90 {
            engine.update();
        }
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
