use std::ffi::{OsStr, OsString};
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn background_command(program: impl AsRef<OsStr>) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new(program)
    }
}

fn configured_program(variable: &str) -> Option<OsString> {
    std::env::var_os(variable).filter(|value| !value.is_empty())
}

pub(crate) fn git_command() -> Command {
    let program = configured_program("CQA_GIT").unwrap_or_else(|| {
        #[cfg(target_os = "windows")]
        if let Some(path) = windows_git_executable() {
            return path.into_os_string();
        }
        OsString::from("git")
    });
    background_command(program)
}

pub(crate) fn claude_command() -> Command {
    let program = configured_program("CQA_CLAUDE").unwrap_or_else(|| {
        #[cfg(target_os = "windows")]
        if let Some(path) = windows_claude_executable() {
            return path.into_os_string();
        }
        OsString::from("claude")
    });
    background_command(program)
}

pub(crate) fn quest_shell_command() -> Option<Command> {
    if let Some(shell) = configured_program("CQA_SHELL") {
        return Some(background_command(shell));
    }

    #[cfg(target_os = "windows")]
    {
        windows_git_bash().map(background_command)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Some(background_command("bash"))
    }
}

#[cfg(target_os = "windows")]
fn executable_on_path(file_name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(file_name))
        .find(|candidate| candidate.is_file())
}

#[cfg(target_os = "windows")]
fn first_installed(
    candidates: impl IntoIterator<Item = std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    candidates.into_iter().find(|candidate| candidate.is_file())
}

#[cfg(target_os = "windows")]
fn windows_git_roots() -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(directory) = std::env::var_os(variable) {
            roots.push(std::path::PathBuf::from(directory).join("Git"));
        }
    }
    if let Some(directory) = std::env::var_os("LOCALAPPDATA") {
        roots.push(
            std::path::PathBuf::from(directory)
                .join("Programs")
                .join("Git"),
        );
    }
    if let Some(directory) = std::env::var_os("USERPROFILE") {
        roots.push(
            std::path::PathBuf::from(directory)
                .join("scoop")
                .join("apps")
                .join("git")
                .join("current"),
        );
    }
    if let Some(directory) = std::env::var_os("ProgramData") {
        roots.push(
            std::path::PathBuf::from(directory)
                .join("scoop")
                .join("apps")
                .join("git")
                .join("current"),
        );
    }
    roots
}

#[cfg(target_os = "windows")]
fn windows_git_executable() -> Option<std::path::PathBuf> {
    executable_on_path("git.exe").or_else(|| {
        first_installed(windows_git_roots().into_iter().flat_map(|root| {
            [
                root.join("cmd").join("git.exe"),
                root.join("bin").join("git.exe"),
            ]
        }))
    })
}

#[cfg(target_os = "windows")]
fn windows_claude_executable() -> Option<std::path::PathBuf> {
    executable_on_path("claude.exe").or_else(|| {
        let home = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from);
        first_installed(
            home.into_iter()
                .map(|directory| directory.join(".local").join("bin").join("claude.exe")),
        )
    })
}

#[cfg(target_os = "windows")]
fn bash_candidates_from(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    path.ancestors()
        .flat_map(|ancestor| {
            [
                ancestor.join("bin").join("bash.exe"),
                ancestor.join("usr").join("bin").join("bash.exe"),
            ]
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn is_windows_subsystem_launcher(path: &std::path::Path) -> bool {
    let normalized = path
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    let in_windows_directory = std::env::var_os("WINDIR")
        .map(|directory| {
            std::path::PathBuf::from(directory)
                .to_string_lossy()
                .replace('/', "\\")
                .trim_end_matches('\\')
                .to_ascii_lowercase()
        })
        .is_some_and(|directory| {
            normalized == directory || normalized.starts_with(&format!("{directory}\\"))
        });
    in_windows_directory || normalized.contains("\\microsoft\\windowsapps\\")
}

#[cfg(target_os = "windows")]
fn windows_git_bash() -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(output) = git_command().arg("--exec-path").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                candidates.extend(bash_candidates_from(std::path::Path::new(&path)));
            }
        }
    }
    if let Some(git) = windows_git_executable() {
        candidates.extend(bash_candidates_from(&git));
    }
    for root in windows_git_roots() {
        candidates.push(root.join("bin").join("bash.exe"));
        candidates.push(root.join("usr").join("bin").join("bash.exe"));
    }
    if let Some(shell) = first_installed(candidates) {
        return Some(shell);
    }

    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join("bash.exe"))
        .find(|candidate| candidate.is_file() && !is_windows_subsystem_launcher(candidate))
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;

    #[test]
    fn installed_git_for_windows_supplies_git_and_a_posix_shell() {
        let git = git_command()
            .arg("--version")
            .output()
            .expect("Git should be discoverable on the Windows build runner");
        assert!(git.status.success());

        let shell_path = windows_git_bash()
            .expect("Git Bash should be discoverable without falling back to WSL");
        assert!(!is_windows_subsystem_launcher(&shell_path));
        let shell = background_command(shell_path)
            .args(["-c", "printf CODEQUEST_WINDOWS_SHELL"])
            .output()
            .expect("Git Bash should start");
        assert!(shell.status.success());
        assert_eq!(shell.stdout, b"CODEQUEST_WINDOWS_SHELL");
    }
}
