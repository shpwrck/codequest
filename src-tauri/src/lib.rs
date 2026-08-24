// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod codequest;
mod engine;
mod external_tools;
mod font5x7;
mod save;
pub mod scene_machine;

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;
use tauri_plugin_dialog::DialogExt;
use wait_timeout::ChildExt;

use codequest::{CodeQuestConfig, GameType};
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

fn sanitized_metadata(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

fn explicit_copyright_notice(path: &std::path::Path) -> Option<String> {
    const NOTICE_FILES: [&str; 9] = [
        "COPYRIGHT",
        "COPYRIGHT.txt",
        "COPYRIGHT.md",
        "LICENSE",
        "LICENSE.txt",
        "LICENSE.md",
        "NOTICE",
        "NOTICE.txt",
        "NOTICE.md",
    ];

    NOTICE_FILES.iter().find_map(|name| {
        let file = std::fs::File::open(path.join(name)).ok()?;
        let mut bytes = Vec::new();
        file.take(16 * 1024).read_to_end(&mut bytes).ok()?;
        String::from_utf8_lossy(&bytes).lines().find_map(|line| {
            let notice = sanitized_metadata(line, 96);
            let lowercase = notice.to_ascii_lowercase();
            let declares_copyright = lowercase.starts_with("copyright ")
                || lowercase.starts_with("copyright(")
                || lowercase.starts_with("(c) ")
                || lowercase.starts_with("spdx-filecopyrighttext:")
                || notice.starts_with('©');
            declares_copyright.then_some(notice)
        })
    })
}

fn repository_provenance(path: &std::path::Path) -> engine::RepositoryProvenance {
    let authors = git_out(path, &["shortlog", "-sne", "--all"])
        .lines()
        .filter_map(|line| {
            let author = line
                .trim_start()
                .trim_start_matches(|ch: char| ch.is_ascii_digit())
                .trim();
            let name = author
                .rsplit_once(" <")
                .map_or(author, |(name, _)| name)
                .trim();
            let name = sanitized_metadata(name, 64);
            (!name.is_empty()).then_some(name)
        })
        .take(3)
        .collect();

    let years: Vec<u16> = git_out(path, &["log", "--all", "--format=%cd", "--date=format:%Y"])
        .lines()
        .filter_map(|year| year.parse().ok())
        .collect();

    engine::RepositoryProvenance {
        authors,
        first_year: years.iter().min().copied(),
        latest_year: years.iter().max().copied(),
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

struct QuizFile {
    path: String,
    size: u64,
}

struct QuizData {
    files: Vec<QuizFile>,
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

fn quiz_data(path: String) -> Result<QuizData, String> {
    let repo = std::path::Path::new(&path);
    if !is_git_repo(repo) {
        return Err("NOT A GIT REPOSITORY".to_string());
    }
    let mut files: Vec<QuizFile> = git_out(repo, &["ls-files"])
        .lines()
        .take(400)
        .map(|f| QuizFile {
            path: f.to_string(),
            size: std::fs::metadata(repo.join(f))
                .map(|m| m.len())
                .unwrap_or(0),
        })
        .collect();
    files.retain(|f| !f.path.is_empty());
    Ok(QuizData { files })
}

#[derive(Serialize, Deserialize, Clone)]
struct QQuestion {
    q: String,
    choices: Vec<String>,
    answer: usize,
}

const CLAUDE_QUESTION_BATCHES_KEY: &str = "claude.question_batches";

#[derive(Serialize, Deserialize, Clone)]
struct SavedQuestionBatch {
    level: u32,
    questions: Vec<QQuestion>,
}

fn question_is_acceptable(question: &QQuestion) -> bool {
    const FACT_TRIVIA_MARKERS: [&str; 27] = [
        "WHICH FILE",
        "WHAT FILE",
        "WHERE DOES",
        "HOW MANY",
        "FILE NAME",
        "FILENAME",
        "FILE",
        "FILES",
        "DIRECTORY",
        "DIRECTORIES",
        "FOLDER",
        "FOLDERS",
        "PATH",
        "PATHS",
        "EXTENSION",
        "EXTENSIONS",
        "README",
        "COMMIT",
        "COMMITS",
        "BRANCH",
        "AUTHOR",
        "AUTHORS",
        "CONTRIBUTOR",
        "CONTRIBUTORS",
        "LATEST",
        "MOST RECENT",
        "BYTES",
    ];
    const FILE_EXTENSIONS: [&str; 14] = [
        ".RS", ".JS", ".MJS", ".TS", ".TSX", ".PY", ".GO", ".RB", ".JAVA", ".C", ".CPP", ".SH",
        ".CSS", ".HTML",
    ];

    if !engine::quiz_question_fits(&question.q, &question.choices, question.answer) {
        return false;
    }
    let content = std::iter::once(question.q.as_str())
        .chain(question.choices.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase();
    let normalized = content
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let padded = format!(" {normalized} ");
    !FACT_TRIVIA_MARKERS
        .iter()
        .any(|marker| padded.contains(&format!(" {marker} ")))
        && !FILE_EXTENSIONS
            .iter()
            .any(|extension| content.contains(extension))
        && !content.contains('/')
        && !content.contains('\\')
}

fn retain_acceptable_questions(questions: Vec<QQuestion>) -> Vec<QQuestion> {
    questions
        .into_iter()
        .filter(question_is_acceptable)
        .collect()
}

fn accepted_question_batch(
    questions: Vec<QQuestion>,
    expected_count: usize,
) -> Option<Vec<QQuestion>> {
    let valid = retain_acceptable_questions(questions)
        .into_iter()
        .take(expected_count)
        .collect::<Vec<_>>();
    (!valid.is_empty()).then_some(valid)
}

fn load_saved_question_batches(path: &std::path::Path) -> Result<Vec<SavedQuestionBatch>, String> {
    let save = save::SaveFile::open_or_create(path)?;
    Ok(save
        .get::<Vec<SavedQuestionBatch>>(CLAUDE_QUESTION_BATCHES_KEY)
        .unwrap_or_default()
        .into_iter()
        .filter(|batch| {
            batch.level > 0
                && !batch.questions.is_empty()
                && batch.questions.iter().all(question_is_acceptable)
        })
        .collect())
}

fn persist_claude_question_batch(
    path: &std::path::Path,
    level: u32,
    questions: &[QQuestion],
) -> Result<(), String> {
    if level == 0 || questions.is_empty() || !questions.iter().all(question_is_acceptable) {
        return Err("INVALID CLAUDE QUESTION BATCH".to_string());
    }
    let mut save = save::SaveFile::open_or_create(path)?;
    let mut batches = save
        .get::<Vec<SavedQuestionBatch>>(CLAUDE_QUESTION_BATCHES_KEY)
        .unwrap_or_default();
    batches.push(SavedQuestionBatch {
        level,
        questions: questions.to_vec(),
    });
    save.set(CLAUDE_QUESTION_BATCHES_KEY, &batches)
}

fn gather_quiz_data(path: &std::path::Path) -> Result<QuizData, String> {
    quiz_data(path.to_string_lossy().to_string())
}

fn text_excerpt(path: &std::path::Path, max_lines: usize) -> String {
    std::fs::read_to_string(path)
        .map(|t| t.lines().take(max_lines).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

fn claude_question_prompt(
    project_name: &str,
    level: u32,
    count: usize,
    documentation: &str,
    implementation_excerpts: &str,
) -> String {
    format!(
        "You write questions for a retro handheld quiz game about a software project. Generate exactly {count} multiple-choice questions at difficulty level {level} (1=purpose and responsibilities, 3=component interactions and tradeoffs, 5=subtle invariants and design rationale).\n\nCONCEPTS ONLY: test the project's architecture, purpose, domain model, component responsibilities, interactions, invariants, tradeoffs, design rationale, or enduring behavior. Every question must still make sense if the project were reorganized and all implementation locations changed.\n\nNEVER ask about file names, paths, directories, or extensions; where code lives; repository structure; counts, sizes, or lines; dates or times; branches or commits; authors or contributors; ordering or recency; or any other state-in-time fact. Never use those facts as choices.\n\nDISPLAY LIMITS: each question must wrap into at most 4 lines of 31 characters. Each choice must be at most 31 characters. Return exactly 4 non-empty, distinct choices and exactly one correct answer. Wrong choices must be plausible concepts. Do not truncate words or sentences. Do not repeat questions.\n\nRespond with ONLY a JSON array, no prose and no code fences: [{{\"q\":\"...\",\"choices\":[\"a\",\"b\",\"c\",\"d\"],\"answer\":0}}]\n\nPROJECT: {project_name}\nPROJECT DOCUMENTATION:\n{documentation}\nANONYMIZED IMPLEMENTATION EXCERPTS:\n{implementation_excerpts}",
    )
}

fn command_output_with_timeout(
    mut command: Command,
    timeout: Duration,
) -> Result<Output, &'static str> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "CLAUDE CLI UNAVAILABLE")?;
    let stdout = child.stdout.take().ok_or("CLAUDE CALL FAILED")?;
    let stderr = child.stderr.take().ok_or("CLAUDE CALL FAILED")?;
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

    let status = match child
        .wait_timeout(timeout)
        .map_err(|_| "CLAUDE CALL FAILED")?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err("CLAUDE CALL TIMED OUT");
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "CLAUDE CALL FAILED")?
        .map_err(|_| "CLAUDE CALL FAILED")?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "CLAUDE CALL FAILED")?
        .map_err(|_| "CLAUDE CALL FAILED")?;
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
) -> Result<Vec<QQuestion>, String> {
    let d = gather_quiz_data(path)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // The file inventory is used only to select representative source excerpts.
    // Paths and repository metadata are deliberately withheld from Claude so the
    // resulting questions focus on enduring concepts rather than file trivia.
    let mut files: Vec<&QuizFile> = d.files.iter().collect();
    files.sort_by_key(|file| std::cmp::Reverse(file.size));
    let readme = text_excerpt(&path.join("README.md"), 40);
    let src_exts = [
        ".rs", ".js", ".ts", ".py", ".go", ".java", ".c", ".cpp", ".rb", ".sh", ".css", ".html",
    ];
    let mut excerpts = String::new();
    let mut used = 0;
    for f in files.iter() {
        if used >= 2 || f.size > 200_000 {
            continue;
        }
        if src_exts.iter().any(|e| f.path.ends_with(e)) {
            excerpts.push_str("\n--- IMPLEMENTATION EXCERPT ---\n");
            excerpts.push_str(&text_excerpt(&path.join(&f.path), 50));
            excerpts.push('\n');
            used += 1;
        }
    }
    let prompt = claude_question_prompt(&name, level, count, &readme, &excerpts);
    let mut cmd = external_tools::claude_command();
    cmd.args(["-p", &prompt, "--output-format", "json"]);
    if let Ok(model) = std::env::var("CQA_CLAUDE_MODEL") {
        if !model.is_empty() {
            cmd.args(["--model", &model]);
        }
    }
    let out = command_output_with_timeout(cmd, Duration::from_secs(120)).map_err(str::to_string)?;
    if !out.status.success() {
        return Err("CLAUDE CALL FAILED".to_string());
    }
    let envelope: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|_| "BAD CLI OUTPUT".to_string())?;
    let result = envelope
        .get("result")
        .and_then(|r| r.as_str())
        .unwrap_or("");
    let start = result.find('[').ok_or("NO JSON IN RESPONSE")?;
    let end = result.rfind(']').ok_or("NO JSON IN RESPONSE")?;
    let parsed: Vec<QQuestion> = serde_json::from_str(&result[start..=end])
        .map_err(|_| "UNPARSEABLE QUESTIONS".to_string())?;
    accepted_question_batch(parsed, count)
        .ok_or_else(|| "INCOMPLETE OR INVALID QUESTIONS".to_string())
}

fn generate_and_save_questions(
    path: &std::path::Path,
    level: u32,
    count: usize,
) -> Result<Vec<QQuestion>, String> {
    let questions = ai_questions(path, level, count)?;
    persist_claude_question_batch(path, level, &questions)?;
    Ok(questions)
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
    let saved_batches = load_saved_question_batches(std::path::Path::new(&cartridge.path))?;
    let mut questions = Vec::new();
    let mut question_batch_ends = Vec::with_capacity(saved_batches.len());
    for batch in saved_batches {
        questions.extend(
            batch
                .questions
                .into_iter()
                .map(|question| engine::QuizQuestion {
                    question: question.q,
                    choices: question.choices,
                    answer: question.answer,
                }),
        );
        question_batch_ends.push(questions.len());
    }

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
        questions,
        question_batch_ends,
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
fn engine_power(state: State<EngineState>, powered: bool) -> Result<(), String> {
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
fn engine_frame(state: State<EngineState>) -> tauri::ipc::Response {
    tauri::ipc::Response::new(state.0.frame())
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
    let question_loader: engine::QuestionLoader = Arc::new(|path, level, count| {
        if environment_flag_enabled(std::env::var("CQA_NO_AI").ok().as_deref()) {
            return Vec::new();
        }
        generate_and_save_questions(std::path::Path::new(&path), level, count)
            .unwrap_or_default()
            .into_iter()
            .map(|question| engine::QuizQuestion {
                question: question.q,
                choices: question.choices,
                answer: question.answer,
            })
            .collect()
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(EngineState(engine::EngineRuntime::spawn(question_loader)))
        .invoke_handler(tauri::generate_handler![
            pick_cartridge,
            cartridge_branch,
            engine_set_cartridge,
            engine_power,
            engine_finish_boot,
            engine_input,
            engine_frame
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod question_policy_tests {
    use super::*;

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

    fn question(text: &str, choices: &[&str]) -> QQuestion {
        QQuestion {
            q: text.to_string(),
            choices: choices.iter().map(|choice| (*choice).to_string()).collect(),
            answer: 0,
        }
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

        let error = command_output_with_timeout(command, Duration::from_millis(50)).unwrap_err();

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
    fn accepted_ai_questions_are_conceptual_and_fit_the_quiz_layout() {
        let conceptual = question(
            "WHY SEPARATE GAME STATE FROM THE UI?",
            &[
                "TO KEEP RESPONSIBILITIES CLEAR",
                "TO HIDE FAILURES",
                "TO COUPLE COMPONENTS",
                "TO DUPLICATE STATE",
            ],
        );
        assert!(question_is_acceptable(&conceptual));

        let file_trivia = question(
            "WHICH FILE OWNS GAME STATE?",
            &["engine.rs", "main.js", "styles.css", "README.md"],
        );
        assert!(!question_is_acceptable(&file_trivia));

        let state_trivia = question(
            "HOW MANY FILES ARE IN THE REPOSITORY?",
            &["ONE", "TWO", "THREE", "FOUR"],
        );
        assert!(!question_is_acceptable(&state_trivia));

        let overflowing = question(
            &"CONCEPTUAL WORD ".repeat(40),
            &[
                "THIS CHOICE IS LONGER THAN THE DISPLAY CAN POSSIBLY SHOW",
                "SECOND",
                "THIRD",
                "FOURTH",
            ],
        );
        assert!(!question_is_acceptable(&overflowing));
    }

    #[test]
    fn quiz_cartridges_have_no_preloaded_questions() {
        let repo = temporary_git_repo();
        let spec = engine_cartridge(build_cartridge(&repo).unwrap()).unwrap();

        assert!(spec.questions.is_empty());
        remove_temporary_repo(repo);
    }

    #[test]
    fn cartridge_reload_restores_saved_claude_batches() {
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
    fn duplicate_choices_are_rejected() {
        let ambiguous = question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &[
                "THE GAME ENGINE",
                "THE GAME ENGINE",
                "THE VIEW",
                "THE DEVICE SHELL",
            ],
        );

        assert!(!question_is_acceptable(&ambiguous));
    }

    #[test]
    fn claude_results_are_rejected_instead_of_truncated() {
        let valid = question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &[
                "THE GAME ENGINE",
                "THE DEVICE SHELL",
                "THE STYLES",
                "THE VIEW",
            ],
        );
        let file_trivia = question(
            "WHICH FILE DEFINES THE ENGINE?",
            &["engine.rs", "main.js", "styles.css", "README.md"],
        );
        let overflowing = question(
            "WHY KEEP OUTPUT WITHIN A FIXED PRESENTATION BOUNDARY?",
            &[
                "THIS RESPONSE CANNOT FIT IN THE AVAILABLE CHOICE ROW",
                "SECOND",
                "THIRD",
                "FOURTH",
            ],
        );

        let accepted = retain_acceptable_questions(vec![valid.clone(), file_trivia, overflowing]);
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].q, valid.q);
        assert_eq!(accepted[0].choices, valid.choices);
    }

    #[test]
    fn valid_questions_survive_a_mixed_claude_batch() {
        let valid = question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &[
                "THE GAME ENGINE",
                "THE DEVICE SHELL",
                "THE STYLES",
                "THE VIEW",
            ],
        );
        let file_trivia = question(
            "WHICH FILE DEFINES THE ENGINE?",
            &["engine.rs", "main.js", "styles.css", "README.md"],
        );

        let accepted = accepted_question_batch(vec![valid.clone(), file_trivia.clone()], 2)
            .expect("the valid question should remain playable");
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].q, valid.q);
        assert!(accepted_question_batch(vec![file_trivia], 1).is_none());
    }

    #[test]
    fn generated_claude_batch_is_saved_without_replacing_other_game_data() {
        let repo = temporary_git_repo();
        let save_path = save::path_for(&repo);
        let mut save = save::SaveFile::open_or_create(&repo).unwrap();
        save.set("quest.progress", &serde_json::json!({ "bosses": 2 }))
            .unwrap();
        let generated = vec![question(
            "WHAT SHOULD OWN GAMEPLAY STATE?",
            &[
                "THE GAME ENGINE",
                "THE DEVICE SHELL",
                "THE STYLES",
                "THE VIEW",
            ],
        )];

        persist_claude_question_batch(&repo, 3, &generated).unwrap();

        let reloaded = save::SaveFile::open_or_create(&repo).unwrap();
        assert_eq!(
            reloaded.get::<serde_json::Value>("quest.progress"),
            Some(serde_json::json!({ "bosses": 2 }))
        );
        let batches = reloaded
            .get::<Vec<SavedQuestionBatch>>(CLAUDE_QUESTION_BATCHES_KEY)
            .unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].level, 3);
        assert_eq!(batches[0].questions[0].q, generated[0].q);

        std::fs::remove_file(save_path).unwrap();
        remove_temporary_repo(repo);
    }

    #[test]
    fn claude_prompt_requests_only_concepts_that_fit_the_display() {
        let prompt = claude_question_prompt(
            "DEMO PROJECT",
            3,
            12,
            "A project that separates its engine from its device shell.",
            "The engine owns state. The adapter translates platform operations.",
        );

        assert!(prompt.contains("CONCEPTS ONLY"));
        assert!(prompt.contains("NEVER ask about file names, paths, directories, or extensions"));
        assert!(prompt.contains("still make sense if the project were reorganized"));
        assert!(prompt.contains("at most 4 lines of 31 characters"));
        assert!(prompt.contains("at most 31 characters"));
        assert!(!prompt.contains("FILES:"));
        assert!(!prompt.contains("COMMIT MESSAGES"));
    }
}
