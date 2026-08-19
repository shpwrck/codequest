// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize, Deserialize, Clone)]
struct Quest {
    id: String,
    name: String,
    description: String,
    boss: String,
    command: String,
}

#[derive(Serialize, Clone)]
struct OutputPayload {
    line: String,
    stream: String,
}

#[derive(Serialize, Clone)]
struct DonePayload {
    code: Option<i32>,
    success: bool,
}

struct QuestState(Arc<Mutex<Option<Child>>>);

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
    let mode = if canon.join("CODEQUEST.md").exists() { "custom" } else { "quiz" };
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

#[tauri::command]
fn load_cartridge(path: String) -> Result<Cartridge, String> {
    build_cartridge(std::path::Path::new(&path))
}

#[derive(Serialize, Clone)]
struct QuizFile {
    path: String,
    size: u64,
}

#[derive(Serialize, Clone)]
struct QuizCommit {
    hash: String,
    author: String,
    msg: String,
}

#[derive(Serialize, Clone)]
struct QuizData {
    branch: String,
    total_commits: u64,
    files: Vec<QuizFile>,
    commits: Vec<QuizCommit>,
    authors: Vec<(String, u64)>,
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

#[tauri::command]
fn quiz_data(path: String) -> Result<QuizData, String> {
    let repo = std::path::Path::new(&path);
    if !is_git_repo(repo) {
        return Err("NOT A GIT REPOSITORY".to_string());
    }
    let branch = git_out(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    let total_commits = git_out(repo, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse::<u64>()
        .unwrap_or(0);
    let mut files: Vec<QuizFile> = git_out(repo, &["ls-files"])
        .lines()
        .take(400)
        .map(|f| QuizFile {
            path: f.to_string(),
            size: std::fs::metadata(repo.join(f)).map(|m| m.len()).unwrap_or(0),
        })
        .collect();
    files.retain(|f| !f.path.is_empty());
    let commits: Vec<QuizCommit> = git_out(repo, &["log", "--pretty=format:%h\x1f%an\x1f%s", "-40"])
        .lines()
        .filter_map(|l| {
            let mut it = l.split('\x1f');
            Some(QuizCommit {
                hash: it.next()?.to_string(),
                author: it.next()?.to_string(),
                msg: it.next()?.to_string(),
            })
        })
        .collect();
    let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for a in git_out(repo, &["log", "--pretty=%an", "-500"]).lines() {
        *counts.entry(a.to_string()).or_insert(0) += 1;
    }
    let mut authors: Vec<(String, u64)> = counts.into_iter().collect();
    authors.sort_by(|a, b| b.1.cmp(&a.1));
    authors.truncate(8);
    Ok(QuizData {
        branch,
        total_commits,
        files,
        commits,
        authors,
    })
}

#[derive(Serialize, Deserialize, Clone)]
struct QQuestion {
    q: String,
    choices: Vec<String>,
    answer: usize,
}

#[derive(Serialize, Clone)]
struct QuizBatch {
    questions: Vec<QQuestion>,
    source: String,
}

fn seeded(n: &mut u64) -> u64 {
    *n = n.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*n >> 33) ^ *n
}

fn pick_idx(rng: &mut u64, len: usize) -> usize {
    (seeded(rng) as usize) % len.max(1)
}

fn fake_name(real: &str, hard: bool, rng: &mut u64) -> String {
    let (dir, base) = match real.rfind('/') {
        Some(i) => (&real[..=i], &real[i + 1..]),
        None => ("", real),
    };
    let (stem, ext) = match base.rfind('.') {
        Some(i) if i > 0 => (&base[..i], &base[i..]),
        _ => (base, ""),
    };
    let variants: Vec<String> = if hard {
        vec![
            format!("{stem}s{ext}"),
            format!("{}{ext}", &stem[..stem.len().saturating_sub(1).max(1)]),
            format!("{stem}{}", if ext == ".js" { ".mjs" } else { ".js" }),
            format!("{stem}-v2{ext}"),
        ]
    } else {
        vec![
            format!("{stem}-old{ext}"),
            format!("my-{stem}{ext}"),
            format!("{stem}.bak"),
            format!("{stem}2{ext}"),
        ]
    };
    format!("{dir}{}", variants[pick_idx(rng, variants.len())])
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
    QQuestion { q: q.to_string(), choices, answer }
}

fn procedural_questions(d: &QuizData, level: u32, count: usize) -> Vec<QQuestion> {
    let mut rng: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|t| t.as_nanos() as u64)
        .unwrap_or(42);
    let hard = level >= 3;
    let mut out = Vec::new();
    // dominant language by extension (stable stack identity, asked thematically)
    let lang_of = |ext: &str| match ext {
        ".rs" => Some("RUST"),
        ".js" | ".mjs" => Some("JAVASCRIPT"),
        ".ts" | ".tsx" => Some("TYPESCRIPT"),
        ".py" => Some("PYTHON"),
        ".go" => Some("GO"),
        ".rb" => Some("RUBY"),
        ".java" | ".kt" => Some("JAVA/KOTLIN"),
        ".c" | ".h" | ".cpp" => Some("C/C++"),
        ".sh" => Some("SHELL"),
        _ => None,
    };
    let mut lang_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for f in &d.files {
        if let Some(i) = f.path.rfind('.') {
            if let Some(l) = lang_of(&f.path[i..]) {
                *lang_counts.entry(l).or_insert(0) += 1;
            }
        }
    }
    let mut langs: Vec<(&str, usize)> = lang_counts.into_iter().collect();
    langs.sort_by(|a, b| b.1.cmp(&a.1));
    // top-level dirs for structure questions
    let dirs: Vec<String> = {
        let mut set = std::collections::HashSet::new();
        for f in &d.files {
            if let Some(i) = f.path.find('/') {
                set.insert(f.path[..i].to_string());
            }
        }
        set.into_iter().collect()
    };
    for _ in 0..count * 4 {
        if out.len() >= count {
            break;
        }
        let q = match pick_idx(&mut rng, 5) {
            0 if d.files.len() >= 4 => {
                let real = d.files[pick_idx(&mut rng, d.files.len())].path.clone();
                let mut fakes = Vec::new();
                for _ in 0..8 {
                    let f = fake_name(&d.files[pick_idx(&mut rng, d.files.len())].path, hard, &mut rng);
                    if !d.files.iter().any(|g| g.path == f) && !fakes.contains(&f) {
                        fakes.push(f);
                    }
                    if fakes.len() == 3 { break; }
                }
                if fakes.len() < 3 { continue; }
                mk_q("WHICH FILE IS PART OF THIS PROJECT?", real, fakes, &mut rng)
            }
            1 if d.files.len() >= 4 => {
                let mut idx: Vec<usize> = (0..d.files.len()).collect();
                for i in (1..idx.len()).rev() { let j = pick_idx(&mut rng, i + 1); idx.swap(i, j); }
                let three: Vec<String> = idx.iter().take(3).map(|i| d.files[*i].path.clone()).collect();
                let fake = fake_name(&d.files[pick_idx(&mut rng, d.files.len())].path, true, &mut rng);
                if d.files.iter().any(|g| g.path == fake) { continue; }
                mk_q("WHICH FILE IS NOT PART OF THIS PROJECT?", fake, three, &mut rng)
            }
            2 if !d.commits.is_empty() => {
                let real: String = d.commits[pick_idx(&mut rng, d.commits.len())].msg.chars().take(34).collect();
                let fakes: Vec<String> = ["FIX TYPO IN README", "UPDATE DEPENDENCIES", "REFACTOR UTILS", "BUMP VERSION", "REMOVE DEAD CODE"]
                    .iter().map(|s| s.to_string()).collect();
                mk_q("WHICH IS REAL PROJECT HISTORY?", real, fakes, &mut rng)
            }
            3 if dirs.len() >= 2 => {
                let nested: Vec<&QuizFile> = d.files.iter().filter(|f| f.path.contains('/')).collect();
                if nested.is_empty() { continue; }
                let f = nested[pick_idx(&mut rng, nested.len())];
                let dir = f.path[..f.path.find('/').unwrap()].to_string();
                let base = f.path[f.path.rfind('/').unwrap() + 1..].to_string();
                let others: Vec<String> = dirs.iter().filter(|x| **x != dir).cloned().collect();
                if others.is_empty() { continue; }
                mk_q(&format!("WHERE DOES {base} LIVE?"), dir, others, &mut rng)
            }
            4 if langs.len() >= 1 => {
                let top = langs[0].0.to_string();
                let others: Vec<String> = ["RUST", "JAVASCRIPT", "TYPESCRIPT", "PYTHON", "GO", "RUBY"]
                    .iter().map(|s| s.to_string()).filter(|l| *l != top).collect();
                mk_q("WHICH LANGUAGE ANCHORS THIS PROJECT?", top, others, &mut rng)
            }
            _ => continue,
        };
        if q.choices.len() >= 2 {
            out.push(q);
        }
    }
    out
}

fn gather_quiz_data(path: &std::path::Path) -> Result<QuizData, String> {
    quiz_data(path.to_string_lossy().to_string())
}

fn text_excerpt(path: &std::path::Path, max_lines: usize) -> String {
    std::fs::read_to_string(path)
        .map(|t| t.lines().take(max_lines).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

fn ai_questions(path: &std::path::Path, level: u32, count: usize) -> Result<Vec<QQuestion>, String> {
    let d = gather_quiz_data(path)?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    // context: files, commits, readme + two biggest source files (excerpts)
    let mut files: Vec<&QuizFile> = d.files.iter().collect();
    files.sort_by(|a, b| b.size.cmp(&a.size));
    let file_list: String = d.files.iter().take(60)
        .map(|f| format!("{} ({}b)", f.path, f.size))
        .collect::<Vec<_>>().join("\n");
    let commit_list: String = d.commits.iter().take(20)
        .map(|c| format!("{} {} ({})", c.hash, c.msg, c.author))
        .collect::<Vec<_>>().join("\n");
    let readme = text_excerpt(&path.join("README.md"), 40);
    let src_exts = [".rs", ".js", ".ts", ".py", ".go", ".java", ".c", ".cpp", ".rb", ".sh", ".css", ".html"];
    let mut excerpts = String::new();
    let mut used = 0;
    for f in files.iter() {
        if used >= 2 || f.size > 200_000 { continue; }
        if src_exts.iter().any(|e| f.path.ends_with(e)) {
            excerpts.push_str(&format!("\n--- {} ---\n{}\n", f.path, text_excerpt(&path.join(&f.path), 50)));
            used += 1;
        }
    }
    let prompt = format!(
        "You write questions for a retro handheld quiz game about a git repository. Generate exactly {count} multiple-choice questions at difficulty level {level} (1=purpose and what components are for, 3=how components interact and why, 5=subtle design decisions and behavior). THEMATIC ONLY: every question must test understanding of the project's architecture, purpose, domain concepts, component roles, or design decisions — things that stay true as the repo evolves. FORBIDDEN: counts of anything, file or repo sizes, dates or times, current branch, most-recent-commit or ordering questions, contributor statistics, or any state-in-time measurement. Rules: each question text under 70 characters; exactly 4 choices, each under 34 characters; exactly one correct choice; wrong choices must be plausible; questions must be answerable from the material below; no duplicates. Respond with ONLY a JSON array, no prose, no code fences: [{{\"q\":\"...\",\"choices\":[\"a\",\"b\",\"c\",\"d\"],\"answer\":0}}]\n\nREPO: {name}\nFILES:\n{file_list}\nCOMMIT MESSAGES (for context, not for recency questions):\n{commit_list}\nREADME EXCERPT:\n{readme}\nSOURCE EXCERPTS:{excerpts}",
        count = count, level = level, name = name,
        file_list = file_list, commit_list = commit_list,
        readme = readme, excerpts = excerpts,
    );
    let mut cmd = Command::new("timeout");
    cmd.args(["120", "claude", "-p", &prompt, "--output-format", "json"]);
    if let Ok(model) = std::env::var("CQA_CLAUDE_MODEL") {
        if !model.is_empty() {
            cmd.args(["--model", &model]);
        }
    }
    let out = cmd.output().map_err(|_| "CLAUDE CLI UNAVAILABLE".to_string())?;
    if !out.status.success() {
        return Err("CLAUDE CALL FAILED".to_string());
    }
    let envelope: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|_| "BAD CLI OUTPUT".to_string())?;
    let result = envelope.get("result").and_then(|r| r.as_str()).unwrap_or("");
    let start = result.find('[').ok_or("NO JSON IN RESPONSE")?;
    let end = result.rfind(']').ok_or("NO JSON IN RESPONSE")?;
    let parsed: Vec<QQuestion> =
        serde_json::from_str(&result[start..=end]).map_err(|_| "UNPARSEABLE QUESTIONS".to_string())?;
    let valid: Vec<QQuestion> = parsed
        .into_iter()
        .filter(|q| q.choices.len() == 4 && q.answer < 4 && !q.q.is_empty())
        .map(|mut q| {
            q.q.truncate(90);
            for c in q.choices.iter_mut() {
                c.truncate(40);
            }
            q
        })
        .collect();
    if valid.is_empty() {
        return Err("NO VALID QUESTIONS".to_string());
    }
    Ok(valid)
}

#[tauri::command]
async fn quiz_batch(path: String, level: u32, count: usize) -> Result<QuizBatch, String> {
    let repo = std::path::PathBuf::from(&path);
    if !is_git_repo(&repo) {
        return Err("NOT A GIT REPOSITORY".to_string());
    }
    let n = count.clamp(1, 12);
    if std::env::var("CQA_NO_AI").is_err() {
        if let Ok(questions) = ai_questions(&repo, level, n) {
            return Ok(QuizBatch { questions, source: "ORACLE".to_string() });
        }
    }
    let d = gather_quiz_data(&repo)?;
    let questions = procedural_questions(&d, level, n);
    if questions.is_empty() {
        return Err("COULD NOT GENERATE QUESTIONS".to_string());
    }
    Ok(QuizBatch { questions, source: "ARCHIVE".to_string() })
}

#[tauri::command]
fn start_quest(app: AppHandle, state: State<QuestState>, command: String) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|_| "quest state is poisoned".to_string())?;
    if guard.is_some() {
        return Err("A quest is already in progress".to_string());
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(&command)
        .current_dir(home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start quest: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture quest stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture quest stderr".to_string())?;

    *guard = Some(child);
    drop(guard);

    let out_app = app.clone();
    let out_reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = out_app.emit(
                "quest://output",
                OutputPayload {
                    line,
                    stream: "out".to_string(),
                },
            );
        }
    });

    let err_app = app.clone();
    let err_reader = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = err_app.emit(
                "quest://output",
                OutputPayload {
                    line,
                    stream: "err".to_string(),
                },
            );
        }
    });

    // Supervisor: after both pipes close, reap the child, clear the slot, and
    // emit "quest://done" exactly once — this is the only place it is emitted,
    // whether the quest exited on its own or was killed by abort_quest.
    let slot = Arc::clone(&state.0);
    thread::spawn(move || {
        let _ = out_reader.join();
        let _ = err_reader.join();

        let child = slot
            .lock()
            .ok()
            .and_then(|mut running| running.take());
        let (code, success) = match child.map(|mut c| c.wait()) {
            Some(Ok(status)) => (status.code(), status.success()),
            _ => (None, false),
        };
        let _ = app.emit("quest://done", DonePayload { code, success });
    });

    Ok(())
}

#[tauri::command]
fn abort_quest(state: State<QuestState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|_| "quest state is poisoned".to_string())?;
    match guard.as_mut() {
        Some(child) => {
            // Kill in place; the supervisor thread reaps the child, clears the
            // slot, and emits the single "quest://done" (success: false).
            child.kill().map_err(|e| format!("Failed to abort quest: {e}"))
        }
        None => Err("No quest is in progress".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(QuestState(Arc::new(Mutex::new(None))))
        .invoke_handler(tauri::generate_handler![
            pick_cartridge,
            load_cartridge,
            quiz_data,
            quiz_batch,
            start_quest,
            abort_quest
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
