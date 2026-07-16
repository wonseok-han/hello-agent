use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub path: String,
    pub version: String,
    /// 사용자의 로그인 셸 PATH에서 이 도구가 보이는지 (터미널에서 실행 가능 여부)
    pub in_shell_path: bool,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentReport {
    pub os: String,
    pub arch: String,
    pub os_version: Option<String>,
    pub claude: Option<ToolInfo>,
    pub node: Option<ToolInfo>,
    pub checked_paths: Vec<String>,
}

#[tauri::command]
pub async fn detect_environment() -> Result<EnvironmentReport, String> {
    tauri::async_runtime::spawn_blocking(detect)
        .await
        .map_err(|e| e.to_string())
}

pub fn detect() -> EnvironmentReport {
    let home = home_dir();
    let shell_dirs = shell_path_dirs();
    let mut checked = Vec::new();

    let claude = probe(&claude_candidates(&home, &shell_dirs), &shell_dirs, &mut checked);
    let node = probe(&node_candidates(&home, &shell_dirs), &shell_dirs, &mut checked);

    EnvironmentReport {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        os_version: os_version(),
        claude,
        node,
        checked_paths: checked,
    }
}

pub(crate) fn home_dir() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).unwrap_or_default()
}

fn exe(bin: &str) -> String {
    if cfg!(windows) {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    }
}

/// GUI 앱은 터미널과 PATH가 다르므로, 알려진 설치 위치를 절대경로로 우선 확인하고
/// 사용자의 로그인 셸 PATH를 보조로 사용한다 (docs/architecture.md §5).
fn claude_candidates(home: &Path, shell_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = vec![home.join(".local/bin")];
    if !cfg!(windows) {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(home.join(".claude/local"));
        dirs.push(home.join(".npm-global/bin"));
    }
    dirs.extend(shell_dirs.iter().cloned());
    dedup_join(dirs, &exe("claude"))
}

fn node_candidates(home: &Path, shell_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if !cfg!(windows) {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(home.join(".volta/bin"));
        // nvm은 버전별 디렉터리라 가장 최신 버전 하나를 후보에 추가
        if let Some(d) = latest_nvm_bin(home) {
            dirs.push(d);
        }
    }
    dirs.extend(shell_dirs.iter().cloned());
    dedup_join(dirs, &exe("node"))
}

fn dedup_join(dirs: Vec<PathBuf>, bin: &str) -> Vec<PathBuf> {
    let mut seen = Vec::new();
    for d in dirs {
        let p = d.join(bin);
        if !seen.contains(&p) {
            seen.push(p);
        }
    }
    seen
}

fn latest_nvm_bin(home: &Path) -> Option<PathBuf> {
    let versions = home.join(".nvm/versions/node");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&versions)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    entries.pop().map(|p| p.join("bin"))
}

fn probe(
    candidates: &[PathBuf],
    shell_dirs: &[PathBuf],
    checked: &mut Vec<String>,
) -> Option<ToolInfo> {
    for path in candidates {
        checked.push(path.display().to_string());
        if !path.is_file() {
            continue;
        }
        if let Some(version) = run_version(path) {
            let in_shell_path = path
                .parent()
                .map(|dir| shell_dirs.iter().any(|d| d == dir))
                .unwrap_or(false);
            return Some(ToolInfo {
                path: path.display().to_string(),
                version,
                in_shell_path,
            });
        }
    }
    None
}

fn run_version(path: &Path) -> Option<String> {
    let out = command(path).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn command(path: &Path) -> Command {
    let cmd = Command::new(path);
    #[cfg(windows)]
    let cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = cmd;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        c
    };
    cmd
}

/// 사용자의 로그인 셸 기준 PATH 디렉터리 목록.
/// "터미널을 열었을 때 claude가 인식되는가"를 판단하는 근거.
fn shell_path_dirs() -> Vec<PathBuf> {
    let raw = if cfg!(windows) {
        std::env::var("PATH").unwrap_or_default()
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        Command::new(shell)
            .args(["-lc", "printf %s \"$PATH\""])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default())
    };
    let sep = if cfg!(windows) { ';' } else { ':' };
    raw.split(sep)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn os_version() -> Option<String> {
    if cfg!(target_os = "macos") {
        let out = Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn detect_runs_on_this_machine() {
        let report = super::detect();
        println!("{report:#?}");
        assert!(!report.os.is_empty());
        assert!(!report.arch.is_empty());
    }
}
