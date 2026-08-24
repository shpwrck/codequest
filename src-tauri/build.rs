use std::path::{Path, PathBuf};
use std::process::Command;

const REVISION_OVERRIDE: &str = "CQA_BUILD_REVISION";

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR"),
    );
    let repository = manifest_dir
        .parent()
        .expect("src-tauri must live inside the CODE QUEST ADVANCE repository");
    let revision = std::env::var(REVISION_OVERRIDE)
        .ok()
        .and_then(|value| normalize_revision(&value))
        .unwrap_or_else(|| repository_revision(repository));

    println!("cargo:rustc-env=CQA_APP_REVISION={revision}");
    println!("cargo:rerun-if-env-changed={REVISION_OVERRIDE}");
    track_repository_head(repository);
    tauri_build::build()
}

fn normalize_revision(value: &str) -> Option<String> {
    let revision = value.trim().to_ascii_lowercase();
    (revision.len() >= 7 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| revision.chars().take(7).collect())
}

fn git_output(repository: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn repository_revision(repository: &Path) -> String {
    git_output(repository, &["rev-parse", "--short=7", "HEAD"])
        .and_then(|revision| normalize_revision(&revision))
        .expect("CODE QUEST ADVANCE builds require a Git SHA or CQA_BUILD_REVISION")
}

fn track_git_path(repository: &Path, name: &str) {
    let Some(path) = git_output(repository, &["rev-parse", "--git-path", name]) else {
        return;
    };
    let path = PathBuf::from(path);
    let path = if path.is_absolute() {
        path
    } else {
        repository.join(path)
    };
    println!("cargo:rerun-if-changed={}", path.display());
}

fn track_repository_head(repository: &Path) {
    track_git_path(repository, "HEAD");
    track_git_path(repository, "packed-refs");
    if let Some(reference) = git_output(repository, &["symbolic-ref", "--quiet", "HEAD"]) {
        track_git_path(repository, &reference);
    }
}
