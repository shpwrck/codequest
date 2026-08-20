// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod engine;
mod font5x7;

use std::process::{Command, Stdio};
use std::thread;

use serde::{Deserialize, Serialize};
use tauri::State;

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
    color: String,
    path: String,
    mode: String, // "custom" when CODEQUEST.md exists (schema TBD), else "quiz"
    quests: Vec<Quest>,
}

fn shquote(p: &str) -> String {
    format!("'{}'", p.replace('\'', "'\\''"))
}

fn is_git_repo(path: &std::path::Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn build_cartridge(path: &std::path::Path) -> Result<Cartridge, String> {
    let canon = std::fs::canonicalize(path).map_err(|_| "DIRECTORY NOT FOUND".to_string())?;
    if !is_git_repo(&canon) {
        return Err("NOT A GIT REPOSITORY - CARTRIDGE REFUSED".to_string());
    }
    let p = canon.to_string_lossy().to_string();
    let name = canon
        .file_name()
        .map(|n| n.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| "REPO".into());
    let q = shquote(&p);
    let mut quests = vec![
        quest(
            "scry",
            "Scrying Pool",
            "Divine the state of the realm.",
            "Fog of State",
            &format!("git -C {q} status --short --branch"),
        ),
        quest(
            "barrow",
            "The Log Barrow",
            "Disturb the burial mound of history.",
            "History Lich",
            &format!("git -C {q} log --oneline --graph --decorate -12"),
        ),
        quest(
            "marsh",
            "Diff Marsh",
            "Wade through the uncommitted changes.",
            "Drift Serpent",
            &format!("git -C {q} diff --stat; git -C {q} diff --cached --stat; true"),
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
    let mode = if canon.join("CODEQUEST.md").exists() {
        "custom"
    } else {
        "quiz"
    };
    Ok(Cartridge {
        id: p.clone(),
        title: name,
        color: palette[h % palette.len()].to_string(),
        path: p,
        mode: mode.to_string(),
        quests,
    })
}

#[tauri::command]
async fn pick_cartridge() -> Result<Option<Cartridge>, String> {
    let out = Command::new("zenity")
        .args([
            "--file-selection",
            "--directory",
            "--title=SELECT CARTRIDGE (GIT REPO)",
        ])
        // keep the dialog on the app's own display: without this, GTK4 delegates
        // to the xdg-desktop-portal, which renders on the host session instead
        .env("GDK_DEBUG", "no-portals")
        .env("GTK_USE_PORTAL", "0")
        .output()
        .map_err(|_| "FOLDER PICKER UNAVAILABLE (NEEDS ZENITY)".to_string())?;
    if !out.status.success() {
        if out.status.code() == Some(1) {
            return Ok(None); // user cancelled the dialog
        }
        return Err("FOLDER PICKER FAILED TO OPEN".to_string());
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        return Ok(None);
    }
    build_cartridge(std::path::Path::new(&path)).map(Some)
}

struct QuizFile {
    path: String,
    size: u64,
}

struct QuizData {
    files: Vec<QuizFile>,
}

fn git_out(path: &std::path::Path, args: &[&str]) -> String {
    Command::new("git")
        .arg("-C")
        .arg(path)
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
    let received_count = questions.len();
    let valid = retain_acceptable_questions(questions);
    (received_count == expected_count && valid.len() == expected_count).then_some(valid)
}

fn seeded(n: &mut u64) -> u64 {
    *n = n
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*n >> 33) ^ *n
}

fn pick_idx(rng: &mut u64, len: usize) -> usize {
    (seeded(rng) as usize) % len.max(1)
}

fn mk_q(q: &str, correct: String, mut others: Vec<String>, rng: &mut u64) -> QQuestion {
    others.retain(|o| *o != correct);
    others.truncate(3);
    let mut choices = vec![correct.clone()];
    choices.append(&mut others);
    // shuffle
    for i in (1..choices.len()).rev() {
        let j = pick_idx(rng, i + 1);
        choices.swap(i, j);
    }
    let answer = choices.iter().position(|c| *c == correct).unwrap_or(0);
    QQuestion {
        q: q.to_string(),
        choices,
        answer,
    }
}

fn conceptual_fallback_questions(count: usize) -> Vec<QQuestion> {
    const BANK: [(&str, &str, [&str; 3]); 18] = [
        (
            "WHY SEPARATE UI FROM GAME LOGIC?",
            "CLEAR RESPONSIBILITIES",
            ["HIDDEN FAILURES", "DUPLICATED STATE", "TIGHTER COUPLING"],
        ),
        (
            "WHAT SHOULD A TEST PROTECT?",
            "OBSERVABLE BEHAVIOR",
            ["PRIVATE DETAILS", "ACCIDENTAL ORDER", "MANUAL HABITS"],
        ),
        (
            "WHY VALIDATE EXTERNAL INPUT?",
            "PRESERVE SYSTEM INVARIANTS",
            [
                "SKIP ERROR HANDLING",
                "INCREASE COUPLING",
                "HIDE INVALID STATE",
            ],
        ),
        (
            "WHY USE A FIXED UPDATE LOOP?",
            "CONSISTENT STATE CHANGES",
            ["RANDOM TIMING", "UNBOUNDED WORK", "IMPLICIT INPUT"],
        ),
        (
            "WHAT MAKES A MODULE EASY TO CHANGE?",
            "A SMALL STABLE INTERFACE",
            [
                "SHARED INTERNAL STATE",
                "MANY ENTRY POINTS",
                "HIDDEN SIDE EFFECTS",
            ],
        ),
        (
            "WHY HANDLE FAILURE EXPLICITLY?",
            "PREDICTABLE FALLBACKS",
            ["SILENT CORRUPTION", "UNLIMITED RETRIES", "UNKNOWN STATE"],
        ),
        (
            "WHAT DOES ENCAPSULATION PROTECT?",
            "INTERNAL IMPLEMENTATION",
            [
                "GLOBAL MUTATION",
                "CALLER ASSUMPTIONS",
                "DUPLICATE OWNERSHIP",
            ],
        ),
        (
            "WHY KEEP DOMAIN TERMS CONSISTENT?",
            "REDUCE AMBIGUITY",
            [
                "ADD MORE STATES",
                "HIDE RESPONSIBILITY",
                "INCREASE SURPRISE",
            ],
        ),
        (
            "WHAT MUST A CACHE NOT REPLACE?",
            "THE SOURCE OF TRUTH",
            ["FAST LOOKUPS", "TEMPORARY RESULTS", "DERIVED VALUES"],
        ),
        (
            "WHY BOUND TEXT BEFORE RENDERING?",
            "PROTECT THE LAYOUT",
            ["CHANGE GAME STATE", "HIDE ALL CONTENT", "REMOVE VALIDATION"],
        ),
        (
            "WHAT SHOULD ASYNC RESULTS CARRY?",
            "REQUEST IDENTITY",
            ["GLOBAL OWNERSHIP", "RANDOM ORDER", "SHARED MUTATION"],
        ),
        (
            "WHY APPLY DATA AT SAFE BOUNDARIES?",
            "AVOID MID-STATE MUTATION",
            ["DELAY ALL INPUT", "SKIP VALIDATION", "DUPLICATE EVENTS"],
        ),
        (
            "WHY KEEP GAME STATE IN ONE ENGINE?",
            "ONE AUTHORITATIVE OWNER",
            [
                "MORE HIDDEN COUPLING",
                "DUPLICATED RULES",
                "UNORDERED EFFECTS",
            ],
        ),
        (
            "WHAT SHOULD AN ADAPTER TRANSLATE?",
            "PLATFORM OPERATIONS",
            ["DOMAIN OWNERSHIP", "GAMEPLAY RULES", "UNRELATED STATE"],
        ),
        (
            "WHY PREFER DETERMINISTIC INPUT?",
            "REPEATABLE BEHAVIOR",
            ["RANDOM OUTCOMES", "IMPLICIT COMMANDS", "HIDDEN TIMING"],
        ),
        (
            "WHAT DOES A FALLBACK PRESERVE?",
            "THE CORE EXPERIENCE",
            ["THE PRIMARY FAILURE", "INVALID OUTPUT", "UNBOUNDED DELAY"],
        ),
        (
            "WHY USE A FRAMEBUFFER CONTRACT?",
            "STABLE PRESENTATION",
            ["VARIABLE DIMENSIONS", "DOM OWNERSHIP", "IMPLICIT SCALING"],
        ),
        (
            "WHAT MAKES AN INTERFACE DEEP?",
            "SIMPLE USEFUL OPERATIONS",
            [
                "EXPOSED INTERNALS",
                "MANY SPECIAL CASES",
                "DUPLICATE METHODS",
            ],
        ),
    ];

    let mut rng = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_nanos() as u64)
        .unwrap_or(42);
    BANK.iter()
        .take(count)
        .map(|(prompt, correct, distractors)| {
            mk_q(
                prompt,
                (*correct).to_string(),
                distractors
                    .iter()
                    .map(|choice| (*choice).to_string())
                    .collect(),
                &mut rng,
            )
        })
        .collect()
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
        "You write questions for a retro handheld quiz game about a software project. Generate exactly {count} multiple-choice questions at difficulty level {level} (1=purpose and responsibilities, 3=component interactions and tradeoffs, 5=subtle invariants and design rationale).\n\nCONCEPTS ONLY: test the project's architecture, purpose, domain model, component responsibilities, interactions, invariants, tradeoffs, design rationale, or enduring behavior. Every question must still make sense if the project were reorganized and all implementation locations changed.\n\nNEVER ask about file names, paths, directories, or extensions; where code lives; repository structure; counts, sizes, or lines; dates or times; branches or commits; authors or contributors; ordering or recency; or any other state-in-time fact. Never use those facts as choices.\n\nDISPLAY LIMITS: each question must wrap into at most 4 lines of 37 characters. Each choice must be at most 35 characters. Return exactly 4 non-empty, distinct choices and exactly one correct answer. Wrong choices must be plausible concepts. Do not truncate words or sentences. Do not repeat questions.\n\nRespond with ONLY a JSON array, no prose and no code fences: [{{\"q\":\"...\",\"choices\":[\"a\",\"b\",\"c\",\"d\"],\"answer\":0}}]\n\nPROJECT: {project_name}\nPROJECT DOCUMENTATION:\n{documentation}\nANONYMIZED IMPLEMENTATION EXCERPTS:\n{implementation_excerpts}",
    )
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
    let mut cmd = Command::new("timeout");
    cmd.args(["120", "claude", "-p", &prompt, "--output-format", "json"]);
    if let Ok(model) = std::env::var("CQA_CLAUDE_MODEL") {
        if !model.is_empty() {
            cmd.args(["--model", &model]);
        }
    }
    let out = cmd
        .output()
        .map_err(|_| "CLAUDE CLI UNAVAILABLE".to_string())?;
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

fn engine_cartridge(cartridge: Cartridge) -> engine::CartridgeSpec {
    let mode = if cartridge.mode == "custom" {
        engine::CartridgeMode::Custom
    } else {
        engine::CartridgeMode::Quiz
    };
    let questions = if mode == engine::CartridgeMode::Quiz {
        conceptual_fallback_questions(18)
            .into_iter()
            .map(|question| engine::QuizQuestion {
                question: question.q,
                choices: question.choices,
                answer: question.answer,
            })
            .collect()
    } else {
        Vec::new()
    };
    engine::CartridgeSpec {
        id: cartridge.path,
        title: cartridge.title,
        mode,
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
    }
}

#[tauri::command]
fn engine_set_cartridge(
    state: State<EngineState>,
    path: Option<String>,
) -> Result<Option<Cartridge>, String> {
    let cartridge = path
        .map(|path| build_cartridge(std::path::Path::new(&path)))
        .transpose()?;
    let oracle_path = cartridge
        .as_ref()
        .filter(|cartridge| cartridge.mode == "quiz")
        .map(|cartridge| cartridge.path.clone());
    state
        .0
        .set_cartridge(cartridge.clone().map(engine_cartridge))?;

    if let Some(path) = oracle_path.filter(|_| std::env::var("CQA_NO_AI").is_err()) {
        let runtime = state.0.clone();
        thread::spawn(move || {
            let questions = ai_questions(std::path::Path::new(&path), 1, 12)
                .unwrap_or_default()
                .into_iter()
                .map(|question| engine::QuizQuestion {
                    question: question.q,
                    choices: question.choices,
                    answer: question.answer,
                })
                .collect::<Vec<_>>();
            if !questions.is_empty() {
                let _ = runtime.replace_questions(path, questions);
            }
        });
    }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(EngineState(engine::EngineRuntime::spawn()))
        .invoke_handler(tauri::generate_handler![
            pick_cartridge,
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

    fn question(text: &str, choices: &[&str]) -> QQuestion {
        QQuestion {
            q: text.to_string(),
            choices: choices.iter().map(|choice| (*choice).to_string()).collect(),
            answer: 0,
        }
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
    fn conceptual_fallback_is_display_safe() {
        let questions = conceptual_fallback_questions(18);
        assert_eq!(questions.len(), 18);
        for question in &questions {
            assert!(
                question_is_acceptable(question),
                "fallback question violated policy: {}",
                question.q
            );
        }
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
    fn incomplete_claude_batches_do_not_replace_the_fallback() {
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

        assert!(accepted_question_batch(vec![valid.clone(), file_trivia.clone()], 2).is_none());
        assert!(accepted_question_batch(vec![valid, file_trivia], 1).is_none());
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
        assert!(prompt.contains("at most 4 lines of 37 characters"));
        assert!(prompt.contains("at most 35 characters"));
        assert!(!prompt.contains("FILES:"));
        assert!(!prompt.contains("COMMIT MESSAGES"));
    }
}
