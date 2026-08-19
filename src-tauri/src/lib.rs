// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize, Clone)]
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

#[tauri::command]
fn list_quests() -> Vec<Quest> {
    vec![
        quest(
            "warmup",
            "Training Grounds",
            "A safe arena to learn the ropes.",
            "Tutorial Slime",
            r#"for i in $(seq 1 12); do echo "Hero strikes for $((i*3)) damage!"; sleep 0.25; done; echo "Tutorial Slime is defeated!""#,
        ),
        quest(
            "scrying",
            "Scrying Pool",
            "Divine the state of the realm (repo status).",
            "Fog of State",
            "git -C /home/jskrzype/workdir/scratch/code-quest-advance status --short --branch && ls /home/jskrzype/workdir/scratch/code-quest-advance/src",
        ),
        quest(
            "forge",
            "The Crate Forge",
            "Reforge the artifact (cargo check this very app).",
            "Borrow Checker",
            "cd /home/jskrzype/workdir/scratch/code-quest-advance/src-tauri && PKG_CONFIG_PATH=/usr/lib64/pkgconfig:/usr/share/pkgconfig cargo check --color never 2>&1",
        ),
        quest(
            "crypt",
            "Cursed Crypt",
            "A doomed expedition. You will not survive.",
            "Segfault Wraith",
            r#"echo "The wraith attacks!"; sleep 0.5; echo "error: your spell fizzles" >&2; sleep 0.5; exit 1"#,
        ),
    ]
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
            list_quests,
            start_quest,
            abort_quest
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
