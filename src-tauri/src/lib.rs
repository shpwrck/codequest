// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod audio;
mod codequest;
mod engine;
mod external_tools;
mod font5x7;
mod learning;
mod provenance;
mod questions;
mod repo_context;
mod save;
pub mod scene_machine;

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::{mpsc, Arc, RwLock};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri_plugin_dialog::DialogExt;
use wait_timeout::ChildExt;

use codequest::{CodeQuestConfig, GameType};
use provenance::sanitized_metadata;
use questions::QQuestion;
use scene_machine::{SceneMachineDefinition, SceneMachineTemplate};

#[derive(Serialize, Deserialize, Clone)]
struct Quest {
    id: String,
    name: String,
    description: String,
    boss: String,
    command: String,
}

struct EngineState(engine::EngineRuntime);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum AiProvider {
    Claude,
    Codex,
}

impl AiProvider {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            _ => Err("UNKNOWN AI BATTERY PROVIDER".to_string()),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Claude => "CLAUDE",
            Self::Codex => "CODEX",
        }
    }
}

#[derive(Default)]
struct AiProviderSession {
    selected: Option<AiProvider>,
    verified: Option<AiProvider>,
}

#[derive(Clone, Default)]
struct AiProviderState(Arc<RwLock<AiProviderSession>>);

impl AiProviderState {
    fn select(&self, provider: Option<AiProvider>) -> Result<(), String> {
        let mut session = self
            .0
            .write()
            .map_err(|_| "AI BATTERY STATE UNAVAILABLE".to_string())?;
        if session.selected != provider {
            session.verified = None;
        }
        session.selected = provider;
        Ok(())
    }

    fn ready_provider(&self) -> Option<AiProvider> {
        self.0.read().ok().and_then(|session| {
            let provider = session.selected?;
            (session.verified == Some(provider)).then_some(provider)
        })
    }

    fn begin_verification(&self, provider: AiProvider) -> Result<(), String> {
        let mut session = self
            .0
            .write()
            .map_err(|_| "AI BATTERY STATE UNAVAILABLE".to_string())?;
        if session.selected != Some(provider) {
            return Err("INSTALL THE SELECTED AI BATTERIES FIRST".to_string());
        }
        session.verified = None;
        Ok(())
    }

    fn mark_verified(&self, provider: AiProvider) -> Result<(), String> {
        let mut session = self
            .0
            .write()
            .map_err(|_| "AI BATTERY STATE UNAVAILABLE".to_string())?;
        if session.selected != Some(provider) {
            return Err("AI BATTERIES CHANGED DURING CHECK".to_string());
        }
        session.verified = Some(provider);
        Ok(())
    }
}

#[derive(Serialize)]
struct AiProviderVerification {
    provider: AiProvider,
    ready: bool,
}

fn quest(id: &str, name: &str, description: &str, boss: &str, command: &str) -> Quest {
    Quest {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        boss: boss.to_string(),
        command: command.to_string(),
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Cartridge {
    id: String,
    title: String,
    branch: String,
    revision: String,
    color: String,
    path: String,
    mode: String,
    quests: Vec<Quest>,
    #[serde(skip)]
    provenance: engine::RepositoryProvenance,
    #[serde(skip)]
    codequest: Option<CodeQuestConfig>,
}

fn shquote(p: &str) -> String {
    format!("'{}'", p.replace('\'', "'\\''"))
}

fn shell_path(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc}");
        }
        if let Some(drive_path) = path.strip_prefix(r"\\?\") {
            return drive_path.to_string();
        }
    }
    path.into_owned()
}

fn git_repo_command(path: &std::path::Path) -> Command {
    let mut command = external_tools::git_command();
    command
        .arg("-c")
        .arg(format!("safe.directory={}", path.to_string_lossy()))
        .arg("-C")
        .arg(path);
    command
}

fn is_git_repo(path: &std::path::Path) -> bool {
    git_repo_command(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn repository_branch(path: &std::path::Path) -> String {
    git_repo_command(path)
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|branch| sanitized_metadata(&branch, 48))
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "DETACHED HEAD".to_string())
}

fn repository_revision(path: &std::path::Path) -> String {
    git_repo_command(path)
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|revision| sanitized_metadata(&revision, 12).to_ascii_lowercase())
        .filter(|revision| !revision.is_empty())
        .unwrap_or_else(|| "-------".to_string())
}

fn explicit_copyright_notice(path: &std::path::Path) -> Option<String> {
    provenance::NOTICE_FILES.iter().find_map(|name| {
        let file = std::fs::File::open(path.join(name)).ok()?;
        let mut bytes = Vec::new();
        file.take(16 * 1024).read_to_end(&mut bytes).ok()?;
        provenance::copyright_notice_in(&String::from_utf8_lossy(&bytes))
    })
}

fn repository_provenance(path: &std::path::Path) -> engine::RepositoryProvenance {
    let authors = provenance::ranked_authors(&git_out(path, &["shortlog", "-sn", "--all"]), 3);
    let years = |args: &[&str]| -> Vec<u16> {
        git_out(path, args)
            .lines()
            .filter_map(|year| year.trim().parse().ok())
            .collect()
    };
    // Root commits bound the first year and the newest commit bounds the
    // latest, so large histories are not formatted commit by commit.
    let first_years = years(&[
        "log",
        "--all",
        "--max-parents=0",
        "--format=%cd",
        "--date=format:%Y",
    ]);
    let latest_years = years(&["log", "--all", "-1", "--format=%cd", "--date=format:%Y"]);

    engine::RepositoryProvenance {
        authors,
        first_year: first_years.iter().min().copied(),
        latest_year: latest_years.iter().max().copied(),
        copyright: explicit_copyright_notice(path),
    }
}

fn build_cartridge(path: &std::path::Path) -> Result<Cartridge, String> {
    let canon = std::fs::canonicalize(path).map_err(|_| "DIRECTORY NOT FOUND".to_string())?;
    if !is_git_repo(&canon) {
        return Err("NOT A GIT REPOSITORY - CARTRIDGE REFUSED".to_string());
    }
    let codequest = CodeQuestConfig::load(&canon)?;
    save::SaveFile::open_or_create(&canon)?;
    let p = canon.to_string_lossy().to_string();
    let default_name = canon
        .file_name()
        .map(|n| n.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| "REPO".into());
    let name = codequest
        .as_ref()
        .and_then(|config| config.game.title.clone())
        .unwrap_or(default_name);
    let q = shquote(&shell_path(&canon));
    let git = format!("git -c safe.directory={q} -C {q}");
    let mut quests = vec![
        quest(
            "scry",
            "Scrying Pool",
            "Divine the state of the realm.",
            "Fog of State",
            &format!("{git} status --short --branch"),
        ),
        quest(
            "barrow",
            "The Log Barrow",
            "Disturb the burial mound of history.",
            "History Lich",
            &format!("{git} log --oneline --graph --decorate -12"),
        ),
        quest(
            "marsh",
            "Diff Marsh",
            "Wade through the uncommitted changes.",
            "Drift Serpent",
            &format!("{git} diff --stat; {git} diff --cached --stat; true"),
        ),
    ];
    if let Ok(text) = std::fs::read_to_string(canon.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(scripts) = v.get("scripts").and_then(|s| s.as_object()) {
                if scripts.contains_key("lint") {
                    quests.push(quest(
                        "lint",
                        "Lint Gauntlet",
                        "Face the keeper of style.",
                        "Style Basilisk",
                        &format!("cd {q} && npm run lint"),
                    ));
                }
                if scripts.contains_key("build") {
                    quests.push(quest(
                        "forge",
                        "The Forge",
                        "Reforge the artifact from source.",
                        "Bundle Golem",
                        &format!("cd {q} && npm run build"),
                    ));
                }
                if scripts.contains_key("test") {
                    quests.push(quest(
                        "dungeon",
                        "Test Dungeon",
                        "Descend where the tests are flaky.",
                        "Flaky Hydra",
                        &format!("cd {q} && npm test"),
                    ));
                }
            }
        }
    }
    if canon.join("Cargo.toml").exists() {
        quests.push(quest(
            "crates",
            "The Crate Forge",
            "Reforge the artifact.",
            "Borrow Checker",
            &format!("cd {q} && cargo check --color never 2>&1"),
        ));
    }
    if canon.join("Makefile").exists() {
        quests.push(quest(
            "mines",
            "The Make Mines",
            "Survey the tunnels (dry run).",
            "Phony Target",
            &format!("cd {q} && make -n 2>&1 | head -40"),
        ));
    }
    let palette = [
        "#6a6fd1", "#38b764", "#e8a33d", "#b13e53", "#41a6f6", "#a06ee0", "#3ec8b8", "#d17ab0",
    ];
    let h: usize = p
        .bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    let mode = match codequest.as_ref().map(|config| config.game.game_type) {
        Some(GameType::Quiz) => "quiz",
        Some(GameType::Quest) => "custom",
        None if canon.join("CODEQUEST.md").exists() => "custom",
        None => "quiz",
    };
    let provenance = repository_provenance(&canon);
    let branch = repository_branch(&canon);
    let revision = repository_revision(&canon);
    Ok(Cartridge {
        id: p.clone(),
        title: name,
        branch,
        revision,
        color: palette[h % palette.len()].to_string(),
        path: p,
        mode: mode.to_string(),
        quests,
        provenance,
        codequest,
    })
}

#[tauri::command]
async fn pick_cartridge(app: tauri::AppHandle) -> Result<Option<Cartridge>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .set_title("SELECT CARTRIDGE (GIT REPO)")
        .blocking_pick_folder()
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|_| "FOLDER PICKER RETURNED AN INVALID PATH".to_string())?;
    build_cartridge(&path).map(Some)
}

#[tauri::command]
fn cartridge_branch(path: String) -> Result<String, String> {
    let canon = std::fs::canonicalize(path).map_err(|_| "DIRECTORY NOT FOUND".to_string())?;
    if !is_git_repo(&canon) {
        return Err("NOT A GIT REPOSITORY - CARTRIDGE REFUSED".to_string());
    }
    Ok(repository_branch(&canon))
}

fn git_out(path: &std::path::Path, args: &[&str]) -> String {
    git_repo_command(path)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

/// Upper bound on tracked paths considered for the generation brief.
const MAX_TRACKED_FILES: usize = 20_000;

/// Tracked paths, `/`-separated and relative to the repository root.
fn tracked_files(path: &std::path::Path) -> Vec<String> {
    git_out(path, &["ls-files", "-z"])
        .split('\0')
        .filter(|file| !file.is_empty())
        .take(MAX_TRACKED_FILES)
        .map(str::to_string)
        .collect()
}

const AI_PROBE_PROMPT: &str =
    "Reply with exactly CODEQUEST_READY and nothing else. Do not inspect files or use tools.";

fn ai_prompt_command(provider: AiProvider, prompt: &str) -> Command {
    match provider {
        AiProvider::Claude => {
            let mut command = external_tools::claude_command();
            command.args([
                "-p",
                prompt,
                "--output-format",
                "json",
                "--no-session-persistence",
                "--tools",
                "",
            ]);
            if let Ok(model) = std::env::var("CQA_CLAUDE_MODEL") {
                if !model.is_empty() {
                    command.args(["--model", &model]);
                }
            }
            command
        }
        AiProvider::Codex => {
            let mut command = external_tools::codex_command();
            command.args([
                "exec",
                "--ephemeral",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "--color",
                "never",
            ]);
            if let Ok(model) = std::env::var("CQA_CODEX_MODEL") {
                if !model.is_empty() {
                    command.args(["--model", &model]);
                }
            }
            command.arg(prompt);
            command
        }
    }
}

fn ai_response_text(provider: AiProvider, stdout: &[u8]) -> Result<String, String> {
    match provider {
        AiProvider::Claude => {
            let envelope: serde_json::Value =
                serde_json::from_slice(stdout).map_err(|_| "BAD CLAUDE CLI OUTPUT".to_string())?;
            envelope
                .get("result")
                .and_then(|result| result.as_str())
                .map(str::to_string)
                .ok_or_else(|| "BAD CLAUDE CLI OUTPUT".to_string())
        }
        AiProvider::Codex => {
            String::from_utf8(stdout.to_vec()).map_err(|_| "BAD CODEX CLI OUTPUT".to_string())
        }
    }
}

fn command_output_with_timeout(
    mut command: Command,
    timeout: Duration,
    provider_name: &str,
) -> Result<Output, String> {
    let unavailable = || format!("{provider_name} CLI UNAVAILABLE");
    let failed = || format!("{provider_name} CALL FAILED");
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| unavailable())?;
    let stdout = child.stdout.take().ok_or_else(&failed)?;
    let stderr = child.stderr.take().ok_or_else(&failed)?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut reader = stdout;
        reader.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut reader = stderr;
        reader.read_to_end(&mut bytes).map(|_| bytes)
    });

    let status = match child.wait_timeout(timeout).map_err(|_| failed())? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(format!("{provider_name} CALL TIMED OUT"));
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| failed())?
        .map_err(|_| failed())?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| failed())?
        .map_err(|_| failed())?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn ai_questions(
    path: &std::path::Path,
    level: u32,
    count: usize,
    active_provider: Option<AiProvider>,
) -> Result<Vec<QQuestion>, String> {
    if !is_git_repo(path) {
        return Err("NOT A GIT REPOSITORY".to_string());
    }
    let provider = active_provider.ok_or_else(|| "AI BATTERIES NOT VERIFIED".to_string())?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // The brief describes the project through anonymized documentation and
    // component skeletons. Paths and repository metadata are withheld from the
    // provider so questions focus on enduring concepts rather than file trivia.
    let brief = repo_context::project_brief(path, &tracked_files(path));
    let learner = questions::load_learner_state(path).unwrap_or_default();
    let prompt = questions::bounded_ai_question_prompt(&name, level, count, &brief, &learner);
    let cmd = ai_prompt_command(provider, &prompt);
    let out = command_output_with_timeout(cmd, Duration::from_secs(120), provider.name())?;
    if !out.status.success() {
        return Err(format!("{} CALL FAILED", provider.name()));
    }
    let result = ai_response_text(provider, &out.stdout)?;
    questions::parse_generated_questions(&result, count)
}

fn generate_and_save_questions(
    path: &std::path::Path,
    level: u32,
    count: usize,
    provider: AiProvider,
) -> Result<Vec<QQuestion>, String> {
    let questions = ai_questions(path, level, count, Some(provider))?;
    questions::persist_ai_question_batch(path, level, &questions)?;
    Ok(questions)
}

fn load_verified_question_batch_with<F>(
    path: &std::path::Path,
    level: u32,
    count: usize,
    provider_state: &AiProviderState,
    generate: &F,
) -> Vec<QQuestion>
where
    F: Fn(&std::path::Path, u32, usize, AiProvider) -> Result<Vec<QQuestion>, String>,
{
    let Some(provider) = provider_state.ready_provider() else {
        return Vec::new();
    };
    generate(path, level, count, provider).unwrap_or_default()
}

/// The engine's question loader: a fresh verified batch, without questions
/// the player has already retired. Missed questions stay playable.
fn load_new_questions_with<F>(
    path: &std::path::Path,
    level: u32,
    count: usize,
    provider_state: &AiProviderState,
    generate: &F,
) -> Vec<engine::QuizQuestion>
where
    F: Fn(&std::path::Path, u32, usize, AiProvider) -> Result<Vec<QQuestion>, String>,
{
    let generated = load_verified_question_batch_with(path, level, count, provider_state, generate);
    let retired = questions::load_retired_questions(path).unwrap_or_default();
    questions::playable_new_questions(generated, &retired)
}

/// Builds the answer recorder the engine calls when a player commits an
/// answer. The engine calls it from its fixed-rate loop, and a durable save
/// write would stall frames there, so one writer thread applies evidence in
/// commit order. The writer stops once the returned recorder is dropped.
fn answer_recorder_with<F>(persist: F) -> (engine::AnsweredQuestionRecorder, thread::JoinHandle<()>)
where
    F: Fn(&std::path::Path, &learning::AnswerEvidence) + Send + 'static,
{
    let (sender, receiver) = mpsc::channel::<(String, learning::AnswerEvidence)>();
    let writer = thread::Builder::new()
        .name("cqa-save-writer".into())
        .spawn(move || {
            for (path, evidence) in receiver {
                persist(std::path::Path::new(&path), &evidence);
            }
        })
        .expect("failed to start save writer thread");
    let recorder: engine::AnsweredQuestionRecorder = Arc::new(move |path, evidence| {
        let _ = sender.send((path, evidence));
    });
    (recorder, writer)
}

fn engine_cartridge(cartridge: Cartridge) -> Result<engine::CartridgeSpec, String> {
    let mode = if cartridge.mode == "custom" {
        engine::CartridgeMode::Custom
    } else {
        engine::CartridgeMode::Quiz
    };
    let template = match mode {
        engine::CartridgeMode::Quiz => SceneMachineTemplate::Quiz,
        engine::CartridgeMode::Custom => SceneMachineTemplate::Quest,
    };
    let machine = cartridge
        .codequest
        .as_ref()
        .map(CodeQuestConfig::runtime_machine)
        .transpose()?
        .flatten()
        .unwrap_or_else(|| SceneMachineDefinition::template(template));
    let saved = questions::load_cartridge_questions(std::path::Path::new(&cartridge.path))?;

    Ok(engine::CartridgeSpec {
        id: cartridge.path,
        title: cartridge.title,
        mode,
        provenance: cartridge.provenance,
        codequest: cartridge.codequest.map(Box::new),
        machine: Box::new(machine),
        quests: cartridge
            .quests
            .into_iter()
            .map(|quest| engine::QuestSpec {
                name: quest.name,
                boss: quest.boss,
                command: quest.command,
            })
            .collect(),
        questions: saved.questions,
        question_batch_ends: saved.batch_ends,
        question_batch_levels: saved.batch_levels,
        lessons: saved.lessons,
        mastery: saved.mastery,
    })
}

fn prove_ai_provider(provider: AiProvider) -> Result<(), String> {
    let command = ai_prompt_command(provider, AI_PROBE_PROMPT);
    let output = command_output_with_timeout(command, Duration::from_secs(60), provider.name())?;
    if !output.status.success() {
        return Err(format!("{} CALL FAILED", provider.name()));
    }
    let response = ai_response_text(provider, &output.stdout)?;
    if response.trim() != "CODEQUEST_READY" {
        return Err(format!("{} READINESS CHECK FAILED", provider.name()));
    }
    Ok(())
}

#[tauri::command]
fn engine_set_ai_provider(
    engine_state: State<EngineState>,
    provider_state: State<AiProviderState>,
    provider: Option<String>,
) -> Result<(), String> {
    let provider = provider.as_deref().map(AiProvider::parse).transpose()?;
    provider_state.select(provider)?;
    engine_state
        .0
        .set_ai_provider(provider.map(|value| value.name().to_string()))
}

#[tauri::command]
async fn verify_ai_provider(
    provider_state: State<'_, AiProviderState>,
    provider: String,
) -> Result<AiProviderVerification, String> {
    let provider_state = provider_state.inner().clone();
    let provider = AiProvider::parse(&provider)?;
    provider_state.begin_verification(provider)?;
    tauri::async_runtime::spawn_blocking(move || prove_ai_provider(provider))
        .await
        .map_err(|_| format!("{} READINESS CHECK FAILED", provider.name()))??;
    provider_state.mark_verified(provider)?;
    Ok(AiProviderVerification {
        provider,
        ready: true,
    })
}

#[tauri::command]
fn engine_set_cartridge(
    state: State<EngineState>,
    path: Option<String>,
) -> Result<Option<Cartridge>, String> {
    let cartridge = path
        .map(|path| build_cartridge(std::path::Path::new(&path)))
        .transpose()?;
    state
        .0
        .set_cartridge(cartridge.clone().map(engine_cartridge).transpose()?)?;
    Ok(cartridge)
}

#[tauri::command]
fn engine_power(
    state: State<EngineState>,
    provider_state: State<AiProviderState>,
    powered: bool,
) -> Result<(), String> {
    if powered && provider_state.ready_provider().is_none() {
        return Err("AI BATTERIES NOT VERIFIED".to_string());
    }
    state.0.set_power(powered)
}

#[tauri::command]
fn engine_finish_boot(state: State<EngineState>) -> Result<(), String> {
    state.0.finish_boot()
}

#[tauri::command]
fn engine_input(state: State<EngineState>, button: String, pressed: bool) -> Result<(), String> {
    let button = engine::Button::parse(&button).ok_or_else(|| "UNKNOWN BUTTON".to_string())?;
    state.0.input(button, pressed)
}

#[tauri::command]
fn engine_set_reduced_motion(state: State<EngineState>, reduced: bool) -> Result<(), String> {
    state.0.set_reduced_motion(reduced)
}

#[tauri::command]
fn engine_frame(state: State<EngineState>) -> tauri::ipc::Response {
    tauri::ipc::Response::new(state.0.frame())
}

#[tauri::command]
fn engine_audio(state: State<EngineState>) -> audio::AudioBatch {
    state.0.drain_audio()
}

#[tauri::command]
fn app_revision() -> &'static str {
    env!("CQA_APP_REVISION")
}

fn environment_flag_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let provider_state = AiProviderState::default();
    let question_provider_state = provider_state.clone();
    let question_loader: engine::QuestionLoader = Arc::new(move |path, level, count| {
        if environment_flag_enabled(std::env::var("CQA_NO_AI").ok().as_deref()) {
            return Vec::new();
        }
        load_new_questions_with(
            std::path::Path::new(&path),
            level,
            count,
            &question_provider_state,
            &generate_and_save_questions,
        )
    });
    let (answered_question_recorder, _save_writer) = answer_recorder_with(|path, evidence| {
        let _ = questions::persist_answer_evidence(path, evidence);
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(provider_state)
        .manage(EngineState(engine::EngineRuntime::spawn(
            question_loader,
            answered_question_recorder,
        )))
        .invoke_handler(tauri::generate_handler![
            app_revision,
            pick_cartridge,
            cartridge_branch,
            engine_set_ai_provider,
            verify_ai_provider,
            engine_set_cartridge,
            engine_power,
            engine_finish_boot,
            engine_input,
            engine_set_reduced_motion,
            engine_frame,
            engine_audio
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod question_policy_tests {
    use super::*;
    use learning::{AnswerEvidence, Concept};
    use questions::tests::question;

    #[test]
    fn no_ai_flag_requires_an_explicit_truthy_value() {
        for enabled in ["1", "true", "TRUE", "yes", "on", " On "] {
            assert!(environment_flag_enabled(Some(enabled)), "{enabled}");
        }
        for disabled in ["0", "false", "no", "off", "", "anything-else"] {
            assert!(!environment_flag_enabled(Some(disabled)), "{disabled}");
        }
        assert!(!environment_flag_enabled(None));
    }

    #[test]
    fn application_revision_is_a_short_lowercase_git_sha() {
        let revision = app_revision();
        assert_eq!(revision.len(), 7);
        assert!(revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
    }

    #[test]
    fn changing_battery_provider_invalidates_the_previous_readiness_proof() {
        let providers = AiProviderState::default();
        providers.select(Some(AiProvider::Claude)).unwrap();
        assert_eq!(providers.ready_provider(), None);
        providers.mark_verified(AiProvider::Claude).unwrap();
        assert_eq!(providers.ready_provider(), Some(AiProvider::Claude));

        providers.select(Some(AiProvider::Codex)).unwrap();
        assert_eq!(providers.ready_provider(), None);

        providers.select(None).unwrap();
        assert_eq!(providers.ready_provider(), None);
    }

    #[test]
    fn starting_each_power_check_invalidates_the_previous_readiness_proof() {
        let providers = AiProviderState::default();
        providers.select(Some(AiProvider::Claude)).unwrap();
        providers.mark_verified(AiProvider::Claude).unwrap();
        assert_eq!(providers.ready_provider(), Some(AiProvider::Claude));

        providers.begin_verification(AiProvider::Claude).unwrap();
        assert_eq!(providers.ready_provider(), None);
    }

    #[test]
    fn verified_battery_provider_generates_its_own_new_questions() {
        let repo = temporary_git_repo();
        let providers = AiProviderState::default();
        let calls = std::sync::Mutex::new(Vec::new());
        let generate = |path: &std::path::Path, level: u32, _count: usize, provider: AiProvider| {
            calls.lock().unwrap().push(provider);
            let generated = vec![question(
                &format!("WHAT DOES {} GENERATE?", provider.name()),
                &[
                    &format!("NEW {} QUESTIONS", provider.name()),
                    "THE OTHER MODEL'S QUESTIONS",
                    "ONLY SAVED QUESTIONS",
                    "NO QUESTIONS",
                ],
            )];
            questions::persist_ai_question_batch(path, level, &generated)?;
            Ok(generated)
        };

        assert!(load_verified_question_batch_with(&repo, 1, 1, &providers, &generate).is_empty());

        providers.select(Some(AiProvider::Codex)).unwrap();
        assert!(load_verified_question_batch_with(&repo, 1, 1, &providers, &generate).is_empty());
        providers.mark_verified(AiProvider::Codex).unwrap();
        let codex = load_verified_question_batch_with(&repo, 1, 1, &providers, &generate);
        assert_eq!(codex[0].q, "WHAT DOES CODEX GENERATE?");

        providers.select(Some(AiProvider::Claude)).unwrap();
        assert!(load_verified_question_batch_with(&repo, 1, 1, &providers, &generate).is_empty());
        providers.mark_verified(AiProvider::Claude).unwrap();
        let claude = load_verified_question_batch_with(&repo, 1, 1, &providers, &generate);
        assert_eq!(claude[0].q, "WHAT DOES CLAUDE GENERATE?");

        assert_eq!(
            *calls.lock().unwrap(),
            [AiProvider::Codex, AiProvider::Claude]
        );
        let batches = questions::load_saved_question_batches(&repo).unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].questions[0].q, "WHAT DOES CODEX GENERATE?");
        assert_eq!(batches[1].questions[0].q, "WHAT DOES CLAUDE GENERATE?");

        remove_temporary_repo(repo);
    }

    #[test]
    fn each_provider_uses_a_noninteractive_ephemeral_prompt_command() {
        let claude = ai_prompt_command(AiProvider::Claude, "TEST PROMPT");
        let claude_args = claude
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(claude_args
            .windows(2)
            .any(|args| args == ["-p", "TEST PROMPT"]));
        assert!(claude_args.contains(&"--no-session-persistence".to_string()));
        assert!(claude_args.windows(2).any(|args| args == ["--tools", ""]));

        let codex = ai_prompt_command(AiProvider::Codex, "TEST PROMPT");
        let codex_args = codex
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(codex_args.first().map(String::as_str), Some("exec"));
        assert!(codex_args.contains(&"--ephemeral".to_string()));
        assert!(codex_args
            .windows(2)
            .any(|args| args == ["--sandbox", "read-only"]));
        assert_eq!(codex_args.last().map(String::as_str), Some("TEST PROMPT"));
    }

    #[test]
    fn provider_output_adapters_return_only_the_assistant_response() {
        let claude = br#"{"result":"CODEQUEST_READY"}"#;
        assert_eq!(
            ai_response_text(AiProvider::Claude, claude).unwrap(),
            "CODEQUEST_READY"
        );
        assert_eq!(
            ai_response_text(AiProvider::Codex, b"CODEQUEST_READY\n").unwrap(),
            "CODEQUEST_READY\n"
        );
    }

    fn temporary_git_repo() -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "codequest-config-test-{}-{unique}.cartridge",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        let status = external_tools::git_command()
            .arg("-C")
            .arg(&path)
            .args(["init", "--quiet"])
            .status()
            .unwrap();
        assert!(status.success());
        path
    }

    fn remove_temporary_repo(repo: std::path::PathBuf) {
        let _ = std::fs::remove_file(save::path_for(&repo));
        std::fs::remove_dir_all(repo).unwrap();
    }

    fn commit_as(repo: &std::path::Path, name: &str, email: &str, date: &str, message: &str) {
        std::fs::write(repo.join("history.txt"), format!("{message}\n")).unwrap();
        let add = external_tools::git_command()
            .arg("-C")
            .arg(repo)
            .args(["add", "history.txt"])
            .status()
            .unwrap();
        assert!(add.success());
        let commit = external_tools::git_command()
            .arg("-C")
            .arg(repo)
            .args([
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                message,
            ])
            .env("GIT_AUTHOR_NAME", name)
            .env("GIT_AUTHOR_EMAIL", email)
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_NAME", name)
            .env("GIT_COMMITTER_EMAIL", email)
            .env("GIT_COMMITTER_DATE", date)
            .status()
            .unwrap();
        assert!(commit.success());
    }

    #[test]
    #[ignore = "child-process fixture for the timeout test"]
    fn slow_command_fixture() {
        std::thread::sleep(Duration::from_secs(5));
    }

    #[test]
    fn external_commands_are_stopped_at_their_deadline() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--ignored",
            "--exact",
            "question_policy_tests::slow_command_fixture",
        ]);
        let started = std::time::Instant::now();

        let error =
            command_output_with_timeout(command, Duration::from_millis(50), "CLAUDE").unwrap_err();

        assert_eq!(error, "CLAUDE CALL TIMED OUT");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn cartridge_loads_codequest_title_type_and_storyboard() {
        let repo = temporary_git_repo();
        std::fs::write(
            repo.join(codequest::FILE_NAME),
            r#"
                schema_version = 1

                [game]
                type = "quest"
                title = "CONFIGURED ADVENTURE"
            "#,
        )
        .unwrap();

        let cartridge = build_cartridge(&repo).unwrap();
        assert_eq!(cartridge.title, "CONFIGURED ADVENTURE");
        assert_eq!(cartridge.mode, "custom");
        assert!(cartridge.codequest.is_some());

        let spec = engine_cartridge(cartridge).unwrap();
        assert_eq!(spec.title, "CONFIGURED ADVENTURE");
        assert_eq!(spec.mode, engine::CartridgeMode::Custom);
        assert!(spec.codequest.is_some());

        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_load_creates_an_emulator_style_save_file() {
        let repo = temporary_git_repo();
        let save_path = repo.parent().unwrap().join(format!(
            "{}.sav",
            repo.file_name().unwrap().to_string_lossy()
        ));
        assert!(!save_path.exists());

        build_cartridge(&repo).unwrap();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&save_path).unwrap()).unwrap();
        assert_eq!(saved["schema_version"], 1);
        assert_eq!(saved["data"], serde_json::json!({}));

        std::fs::remove_file(save_path).unwrap();
        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_scopes_git_trust_to_the_selected_repository() {
        let repo = temporary_git_repo();
        let canon = std::fs::canonicalize(&repo).unwrap();
        let command = git_repo_command(&canon);
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        assert_eq!(args[0], "-c");
        assert_eq!(args[1], format!("safe.directory={}", canon.display()));
        assert_eq!(args[2], "-C");
        assert_eq!(args[3], canon.to_string_lossy());

        let cartridge = build_cartridge(&repo).unwrap();
        let q = shquote(&shell_path(&canon));
        let git = format!("git -c safe.directory={q} -C {q}");
        assert_eq!(
            cartridge.quests[0].command,
            format!("{git} status --short --branch")
        );
        assert_eq!(
            cartridge.quests[1].command,
            format!("{git} log --oneline --graph --decorate -12")
        );
        assert_eq!(
            cartridge.quests[2].command,
            format!("{git} diff --stat; {git} diff --cached --stat; true")
        );

        std::fs::remove_dir_all(repo).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn shell_paths_remove_windows_extended_length_prefixes() {
        assert_eq!(
            shell_path(std::path::Path::new(r"\\?\C:\repos\code quest")),
            r"C:\repos\code quest"
        );
        assert_eq!(
            shell_path(std::path::Path::new(r"\\?\UNC\server\share\repo")),
            r"\\server\share\repo"
        );
    }

    #[test]
    fn workspace_checkout_uses_scoped_git_trust_when_available() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        if workspace.join(".git").exists() {
            let canon = std::fs::canonicalize(workspace).unwrap();
            assert!(is_git_repo(&canon));
        }
    }

    #[test]
    fn cartridge_reports_the_repositorys_current_branch() {
        let repo = temporary_git_repo();
        let switched = external_tools::git_command()
            .arg("-C")
            .arg(&repo)
            .args(["switch", "--quiet", "-c", "story/cartridge-label"])
            .status()
            .unwrap();
        assert!(switched.success());

        let cartridge = build_cartridge(&repo).unwrap();
        assert_eq!(cartridge.branch, "story/cartridge-label");

        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_reports_the_repositorys_short_head_revision() {
        let repo = temporary_git_repo();
        commit_as(
            &repo,
            "Ada Lovelace",
            "ada@example.com",
            "2024-01-02T12:00:00Z",
            "Create cartridge",
        );
        let expected = git_out(&repo, &["rev-parse", "--short=7", "HEAD"])
            .trim()
            .to_string();

        let cartridge = build_cartridge(&repo).unwrap();
        assert_eq!(cartridge.revision, expected);

        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_compiles_schema_v2_storyboard_for_the_engine() {
        let repo = temporary_git_repo();
        std::fs::write(
            repo.join(codequest::FILE_NAME),
            r#"
                schema_version = 2

                [game]
                type = "quiz"
                start_scene = "title"

                [[scenes]]
                id = "title"
                title = "Title"
                kind = "title"
                handler = "title"

                [[scenes.transitions]]
                signal = "continue"
                target = "game-over"

                [[scenes]]
                id = "game-over"
                title = "Game Over"
                kind = "result"
                handler = "game-over"
            "#,
        )
        .unwrap();

        let cartridge = build_cartridge(&repo).unwrap();
        let spec = engine_cartridge(cartridge).unwrap();
        let mut machine = scene_machine::SceneMachine::new(*spec.machine);
        assert_eq!(machine.current_scene(), "title");
        assert_eq!(
            machine
                .handle(scene_machine::SceneEvent::Signal(
                    scene_machine::SceneSignal::Continue,
                ))
                .unwrap()
                .target,
            "game-over"
        );

        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_loads_ranked_authors_timeline_and_explicit_copyright() {
        let repo = temporary_git_repo();
        commit_as(
            &repo,
            "Ada Lovelace",
            "ada@example.com",
            "2020-01-02T03:04:05Z",
            "first",
        );
        commit_as(
            &repo,
            "Grace Hopper",
            "grace@example.com",
            "2022-02-03T04:05:06Z",
            "second",
        );
        commit_as(
            &repo,
            "Ada Lovelace",
            "ada@example.com",
            "2024-03-04T05:06:07Z",
            "third",
        );
        std::fs::write(
            repo.join("LICENSE"),
            "MIT License\n\nCopyright (c) 2020-2024 Ada Lovelace\n",
        )
        .unwrap();

        let cartridge = build_cartridge(&repo).unwrap();
        assert_eq!(
            cartridge.provenance.authors,
            vec!["Ada Lovelace", "Grace Hopper"]
        );
        assert_eq!(cartridge.provenance.first_year, Some(2020));
        assert_eq!(cartridge.provenance.latest_year, Some(2024));
        assert_eq!(
            cartridge.provenance.copyright.as_deref(),
            Some("Copyright (c) 2020-2024 Ada Lovelace")
        );

        let spec = engine_cartridge(cartridge).unwrap();
        assert_eq!(spec.provenance.authors[0], "Ada Lovelace");
        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_does_not_infer_copyright_from_commit_authors() {
        let repo = temporary_git_repo();
        commit_as(
            &repo,
            "Ada Lovelace",
            "ada@example.com",
            "2020-01-02T03:04:05Z",
            "first",
        );
        std::fs::write(
            repo.join("LICENSE"),
            "The above copyright notice and this permission notice shall be included.\n",
        )
        .unwrap();

        let cartridge = build_cartridge(&repo).unwrap();
        assert_eq!(cartridge.provenance.authors, vec!["Ada Lovelace"]);
        assert_eq!(cartridge.provenance.copyright, None);

        remove_temporary_repo(repo);
    }

    #[test]
    fn quiz_cartridges_have_no_preloaded_questions() {
        let repo = temporary_git_repo();
        let spec = engine_cartridge(build_cartridge(&repo).unwrap()).unwrap();

        assert!(spec.questions.is_empty());
        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_reload_restores_legacy_claude_batches() {
        let repo = temporary_git_repo();
        let save_path = save::path_for(&repo);
        let mut save = save::SaveFile::open_or_create(&repo).unwrap();
        save.set(
            "claude.question_batches",
            &serde_json::json!([
                {
                    "level": 1,
                    "questions": [{
                        "q": "WHAT SHOULD OWN GAMEPLAY STATE?",
                        "choices": [
                            "THE GAME ENGINE",
                            "THE DEVICE SHELL",
                            "THE STYLES",
                            "THE VIEW"
                        ],
                        "answer": 0
                    }]
                },
                {
                    "level": 2,
                    "questions": [{
                        "q": "WHY KEEP THE DEVICE SHELL THIN?",
                        "choices": [
                            "TO CENTRALIZE GAME RULES",
                            "TO DUPLICATE GAME STATE",
                            "TO HIDE ENGINE OUTPUT",
                            "TO BYPASS THE ENGINE"
                        ],
                        "answer": 0
                    }]
                }
            ]),
        )
        .unwrap();

        let spec = engine_cartridge(build_cartridge(&repo).unwrap()).unwrap();

        assert_eq!(spec.questions.len(), 2);
        assert_eq!(
            spec.questions[0].question,
            "WHAT SHOULD OWN GAMEPLAY STATE?"
        );
        assert_eq!(
            spec.questions[1].question,
            "WHY KEEP THE DEVICE SHELL THIN?"
        );
        assert_eq!(spec.question_batch_ends, vec![1, 2]);

        std::fs::remove_file(save_path).unwrap();
        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_reload_resumes_after_last_answered_question() {
        let repo = temporary_git_repo();
        let mut save = save::SaveFile::open_or_create(&repo).unwrap();
        save.set(
            "claude.question_batches",
            &serde_json::json!([
                {
                    "level": 1,
                    "questions": [{
                        "q": "WHAT SHOULD OWN GAMEPLAY STATE?",
                        "choices": [
                            "THE GAME ENGINE",
                            "THE DEVICE SHELL",
                            "THE STYLES",
                            "THE VIEW"
                        ],
                        "answer": 0
                    }]
                },
                {
                    "level": 2,
                    "questions": [{
                        "q": "WHY KEEP THE DEVICE SHELL THIN?",
                        "choices": [
                            "TO CENTRALIZE GAME RULES",
                            "TO DUPLICATE GAME STATE",
                            "TO HIDE ENGINE OUTPUT",
                            "TO BYPASS THE ENGINE"
                        ],
                        "answer": 0
                    }]
                }
            ]),
        )
        .unwrap();
        save.set(
            "quiz.progress",
            &serde_json::json!({
                "answered_questions": ["WHAT SHOULD OWN GAMEPLAY STATE?"]
            }),
        )
        .unwrap();

        let spec = engine_cartridge(build_cartridge(&repo).unwrap()).unwrap();

        assert_eq!(spec.questions.len(), 1);
        assert_eq!(
            spec.questions[0].question,
            "WHY KEEP THE DEVICE SHELL THIN?"
        );
        assert_eq!(spec.question_batch_ends, vec![1]);

        remove_temporary_repo(repo);
    }

    fn answer(question: &str, concept: Option<Concept>, correct: bool) -> AnswerEvidence {
        AnswerEvidence {
            question: question.to_string(),
            concept,
            correct,
            review: false,
        }
    }

    #[test]
    fn cartridge_reload_brings_missed_questions_back_for_review_with_lessons() {
        let repo = temporary_git_repo();
        let explained = question(
            "WHY DOES THE ENGINE OWN STATE?",
            &[
                "ONE OWNER KEEPS RULES FIXED",
                "THE SHELL IS TOO SLOW",
                "STYLES CANNOT HOLD STATE",
                "THE VIEW IS REPLACEABLE",
            ],
        );
        let retired = question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &[
                "THE GAME ENGINE",
                "THE DEVICE SHELL",
                "THE STYLES",
                "THE VIEW",
            ],
        );
        questions::persist_ai_question_batch(&repo, 1, &[retired.clone(), explained.clone()])
            .unwrap();
        questions::persist_answer_evidence(
            &repo,
            &answer(&retired.q, Some(Concept::Responsibility), true),
        )
        .unwrap();
        questions::persist_answer_evidence(
            &repo,
            &answer(&explained.q, Some(Concept::Responsibility), false),
        )
        .unwrap();

        let spec = engine_cartridge(build_cartridge(&repo).unwrap()).unwrap();

        assert_eq!(
            spec.questions.len(),
            1,
            "correct answers retire, misses return"
        );
        let review = &spec.questions[0];
        assert_eq!(review.question, explained.q);
        assert!(review.review);
        assert_eq!(review.concept, Some(Concept::Responsibility));
        assert_eq!(review.rationales, explained.rationales());
        assert_eq!(review.choices, explained.choice_texts());
        assert_eq!(spec.question_batch_ends, vec![1]);
        assert_eq!(
            spec.lessons
                .iter()
                .map(|lesson| (lesson.question.as_str(), lesson.outstanding))
                .collect::<Vec<_>>(),
            [(retired.q.as_str(), false), (explained.q.as_str(), true)]
        );
        assert_eq!(spec.lessons[1].answer, "ONE OWNER KEEPS RULES FIXED");
        assert!(!spec.lessons[1].rationale.is_empty());
        let lens = spec.mastery[&Concept::Responsibility];
        assert_eq!((lens.first_try, lens.missed), (1, 1));

        remove_temporary_repo(repo);
    }

    #[test]
    fn a_wrong_answer_no_longer_retires_a_question() {
        let repo = temporary_git_repo();
        save::SaveFile::open_or_create(&repo)
            .unwrap()
            .set("claude.question_batches", &serde_json::json!([{
                "level": 1,
                "questions": [{
                    "q": "WHAT SHOULD OWN GAMEPLAY STATE?",
                    "choices": ["THE GAME ENGINE", "THE DEVICE SHELL", "THE STYLES", "THE VIEW"],
                    "answer": 0
                }]
            }]))
            .unwrap();

        questions::persist_answer_evidence(
            &repo,
            &answer("WHAT SHOULD OWN GAMEPLAY STATE?", None, false),
        )
        .unwrap();
        let missed = engine_cartridge(build_cartridge(&repo).unwrap()).unwrap();
        assert_eq!(missed.questions.len(), 1);
        assert!(missed.questions[0].review);
        assert!(
            missed.questions[0].rationales.is_empty(),
            "legacy questions have no rationales"
        );
        assert!(missed.lessons[0].outstanding);

        questions::persist_answer_evidence(
            &repo,
            &answer("WHAT SHOULD OWN GAMEPLAY STATE?", None, true),
        )
        .unwrap();
        let redeemed = engine_cartridge(build_cartridge(&repo).unwrap()).unwrap();
        assert!(redeemed.questions.is_empty());
        assert!(!redeemed.lessons[0].outstanding);

        remove_temporary_repo(repo);
    }

    #[test]
    fn the_question_loader_maps_new_batches_and_skips_only_retired_questions() {
        let repo = temporary_git_repo();
        let providers = AiProviderState::default();
        providers.select(Some(AiProvider::Claude)).unwrap();
        providers.mark_verified(AiProvider::Claude).unwrap();
        let retired = question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &[
                "THE GAME ENGINE",
                "THE DEVICE SHELL",
                "THE STYLES",
                "THE VIEW",
            ],
        );
        let missed = question(
            "WHY KEEP THE DEVICE SHELL THIN?",
            &[
                "TO CENTRALIZE GAME RULES",
                "TO DUPLICATE GAME STATE",
                "TO HIDE ENGINE OUTPUT",
                "TO BYPASS THE ENGINE",
            ],
        );
        questions::persist_answer_evidence(&repo, &answer(&retired.q, None, true)).unwrap();
        questions::persist_answer_evidence(&repo, &answer(&missed.q, None, false)).unwrap();
        let batch = vec![retired, missed.clone()];
        let generate = |_: &std::path::Path, _: u32, _: usize, _: AiProvider| Ok(batch.clone());

        let loaded = load_new_questions_with(&repo, 2, 6, &providers, &generate);

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].question, missed.q);
        assert_eq!(loaded[0].concept, missed.lens());
        assert_eq!(loaded[0].rationales, missed.rationales());
        assert!(!loaded[0].review);

        remove_temporary_repo(repo);
    }

    #[test]
    fn the_answer_recorder_persists_evidence_in_commit_order_off_the_engine_thread() {
        let repo = temporary_git_repo();
        let engine_thread = thread::current().id();
        let writer_threads = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = Arc::clone(&writer_threads);
        let (recorder, writer) = answer_recorder_with(move |path, evidence| {
            seen.lock().unwrap().push(thread::current().id());
            questions::persist_answer_evidence(path, evidence).unwrap();
        });
        let cartridge = repo.to_string_lossy().to_string();
        let evidence = |correct| AnswerEvidence {
            review: correct,
            ..answer(
                "WHY WRITE SAVES ATOMICALLY?",
                Some(Concept::Invariant),
                correct,
            )
        };

        recorder(cartridge.clone(), evidence(false));
        recorder(cartridge, evidence(true));
        drop(recorder);
        writer.join().unwrap();

        let progress = questions::load_quiz_progress(&repo).unwrap();
        assert_eq!(progress.answered_questions, ["WHY WRITE SAVES ATOMICALLY?"]);
        assert!(
            progress.missed_questions.is_empty(),
            "the redemption was applied last"
        );
        let lens = progress.mastery[&Concept::Invariant];
        assert_eq!((lens.redeemed, lens.missed), (1, 1));
        assert!(writer_threads
            .lock()
            .unwrap()
            .iter()
            .all(|thread| *thread != engine_thread));

        remove_temporary_repo(repo);
    }

    #[test]
    fn generation_briefs_come_from_tracked_files() {
        let repo = temporary_git_repo();
        std::fs::write(repo.join("README.md"), "# Demo\n\nA tracked overview.\n").unwrap();
        std::fs::write(repo.join("untracked.md"), "# Untracked\n").unwrap();
        let add = external_tools::git_command()
            .arg("-C")
            .arg(&repo)
            .args(["add", "README.md"])
            .status()
            .unwrap();
        assert!(add.success());

        let canon = std::fs::canonicalize(&repo).unwrap();
        assert_eq!(tracked_files(&canon), ["README.md"]);
        let brief = repo_context::project_brief(&canon, &tracked_files(&canon));
        assert!(brief.contains("A tracked overview."));

        remove_temporary_repo(repo);
    }
}
