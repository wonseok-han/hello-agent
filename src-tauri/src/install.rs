use crate::agent::Agent;
use crate::error::AppError;
use serde::Serialize;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use tauri::ipc::Channel;

const CLAUDE_SCRIPT_URL_UNIX: &str = "https://claude.ai/install.sh";
const CLAUDE_SCRIPT_URL_WINDOWS: &str = "https://claude.ai/install.ps1";
const CODEX_RELEASE_BASE: &str = "https://github.com/openai/codex/releases/latest/download";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "type")]
pub enum InstallEvent {
    Phase { name: String },
    Log { line: String },
    /// 지금까지 내려받은 바이트 수
    Progress { downloaded_bytes: u64 },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub version: String,
    pub path: String,
    /// PATH 설정을 추가한 위치 (이미 설정돼 있었으면 None)
    pub profile_updated: Option<String>,
}

/// `test_home`은 격리 검증 전용 (임시 HOME에 설치해 실제 환경을 건드리지 않음).
#[tauri::command]
pub async fn install_agent(
    agent: String,
    test_home: Option<String>,
    on_event: Channel<InstallEvent>,
) -> Result<InstallResult, AppError> {
    let agent = Agent::from_id(&agent)?;
    tauri::async_runtime::spawn_blocking(move || {
        let home = match test_home {
            Some(h) => PathBuf::from(h),
            None => crate::detect::home_dir(),
        };
        run_install(agent, &home, &|e| {
            let _ = on_event.send(e);
        })
    })
    .await
    .map_err(|e| AppError::generic(e.to_string()))?
}

pub fn run_install(
    agent: Agent,
    home: &Path,
    emit: &(dyn Fn(InstallEvent) + Sync),
) -> Result<InstallResult, AppError> {
    let phase = |name: &str| emit(InstallEvent::Phase { name: name.into() });

    match agent {
        Agent::ClaudeCode => install_via_vendor_script(home, emit, &phase)?,
        Agent::Codex => install_via_github_tarball(home, emit, &phase)?,
    }

    // 터미널 PATH 설정 — 두 에이전트 모두 ~/.local/bin에 설치되므로 공통 (docs §5)
    phase("path");
    let profile_updated = ensure_path(home)?;

    // 절대경로로 설치 검증 — PATH에 의존하지 않음
    phase("verify");
    let bin = home
        .join(".local")
        .join("bin")
        .join(crate::detect::exe(agent.bin_name()));
    let out = crate::detect::command(&bin)
        .arg("--version")
        .env(home_env_var(), home)
        .output()
        .map_err(|e| AppError::classify(e.to_string()))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || version.is_empty() {
        return Err(AppError::generic("agent --version check failed after install"));
    }

    Ok(InstallResult {
        version,
        path: bin.display().to_string(),
        profile_updated,
    })
}

/// 클로드 코드: 공식 설치 스크립트를 무인 실행 (스크립트가 다운로드·배치까지 수행)
fn install_via_vendor_script(
    home: &Path,
    emit: &(dyn Fn(InstallEvent) + Sync),
    phase: &dyn Fn(&str),
) -> Result<(), AppError> {
    phase("download");
    let script_dir = home.join(".claude");
    std::fs::create_dir_all(&script_dir).map_err(|e| AppError::classify(e.to_string()))?;
    let (url, script_name) = if cfg!(windows) {
        (CLAUDE_SCRIPT_URL_WINDOWS, "hello-agent-install.ps1")
    } else {
        (CLAUDE_SCRIPT_URL_UNIX, "hello-agent-install.sh")
    };
    let script = script_dir.join(script_name);
    let status = crate::detect::command(&curl_path())
        .args(["-fsSL", url, "-o"])
        .arg(&script)
        .status()
        .map_err(|e| AppError::classify(e.to_string()))?;
    if !status.success() {
        return Err(AppError::network(format!(
            "installer download failed (curl exit {})",
            status.code().unwrap_or(-1)
        )));
    }

    phase("install");
    let mut child = script_command(&script, home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::classify(e.to_string()))?;
    // 스크립트는 진행률을 출력하지 않으므로 다운로드 폴더 크기를 관찰한다
    let status = wait_streaming(&mut child, &home.join(".claude").join("downloads"), emit)
        .map_err(|e| AppError::classify(e.to_string()))?;
    let _ = std::fs::remove_file(&script);
    if !status.success() {
        // exit 코드만으론 원인이 애매 — 프론트 닥터가 스트리밍된 로그로 보완한다
        return Err(AppError::generic(format!(
            "installer exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

/// 코덱스: GitHub 릴리즈의 플랫폼별 바이너리를 받아 ~/.local/bin에 배치
fn install_via_github_tarball(
    home: &Path,
    emit: &(dyn Fn(InstallEvent) + Sync),
    phase: &dyn Fn(&str),
) -> Result<(), AppError> {
    phase("download");
    let target = codex_target()?;
    let url = format!("{CODEX_RELEASE_BASE}/codex-{target}.tar.gz");
    let cache = home.join(".cache").join("hello-agent");
    std::fs::create_dir_all(&cache).map_err(|e| AppError::classify(e.to_string()))?;
    let tarball = cache.join("codex.tar.gz");

    let mut child = crate::detect::command(&curl_path())
        .args(["-fsSL", "--retry", "2", &url, "-o"])
        .arg(&tarball)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::classify(e.to_string()))?;
    let status = wait_streaming(&mut child, &tarball, emit)
        .map_err(|e| AppError::classify(e.to_string()))?;
    if !status.success() {
        return Err(AppError::network(format!(
            "codex download failed (curl exit {})",
            status.code().unwrap_or(-1)
        )));
    }

    phase("install");
    let status = crate::detect::command(&tar_path())
        .arg("-xzf")
        .arg(&tarball)
        .arg("-C")
        .arg(&cache)
        .status()
        .map_err(|e| AppError::classify(e.to_string()))?;
    if !status.success() {
        return Err(AppError::checksum(format!(
            "failed to extract codex archive (tar exit {})",
            status.code().unwrap_or(-1)
        )));
    }

    let inner = cache.join(format!("codex-{target}"));
    let bin_dir = home.join(".local").join("bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| AppError::classify(e.to_string()))?;
    let dest = bin_dir.join(crate::detect::exe("codex"));
    std::fs::rename(&inner, &dest)
        .or_else(|_| std::fs::copy(&inner, &dest).map(|_| ()))
        .map_err(|e| AppError::classify(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&tarball);
    let _ = std::fs::remove_file(&inner);
    Ok(())
}

/// 자식 프로세스의 출력 줄과 다운로드 크기를 이벤트로 흘리며 종료를 기다린다
fn wait_streaming(
    child: &mut Child,
    watch: &Path,
    emit: &(dyn Fn(InstallEvent) + Sync),
) -> std::io::Result<ExitStatus> {
    let mut readers: Vec<Box<dyn Read + Send>> = Vec::new();
    if let Some(o) = child.stdout.take() {
        readers.push(Box::new(o));
    }
    if let Some(e) = child.stderr.take() {
        readers.push(Box::new(e));
    }
    let stop = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|s| {
        for reader in readers {
            s.spawn(move || {
                for line in BufReader::new(reader).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        emit(InstallEvent::Log { line });
                    }
                }
            });
        }
        let (stop_ref, watch) = (&stop, watch.to_path_buf());
        s.spawn(move || {
            let mut last = 0u64;
            while !stop_ref.load(std::sync::atomic::Ordering::Relaxed) {
                let size = path_size(&watch);
                if size > last {
                    last = size;
                    emit(InstallEvent::Progress {
                        downloaded_bytes: size,
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
        let status = child.wait();
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        status
    })
}

fn path_size(p: &Path) -> u64 {
    if p.is_file() {
        return p.metadata().map(|m| m.len()).unwrap_or(0);
    }
    std::fs::read_dir(p)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().and_then(|e| e.metadata().ok()))
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

fn codex_target() -> Result<&'static str, AppError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc.exe"),
        ("windows", "aarch64") => Ok("aarch64-pc-windows-msvc.exe"),
        (os, arch) => Err(AppError::generic(format!(
            "codex install not supported on {os}/{arch}"
        ))),
    }
}

fn home_env_var() -> &'static str {
    if cfg!(windows) { "USERPROFILE" } else { "HOME" }
}

fn curl_path() -> PathBuf {
    if cfg!(windows) {
        system_root().join("System32").join("curl.exe")
    } else {
        PathBuf::from("/usr/bin/curl")
    }
}

fn tar_path() -> PathBuf {
    if cfg!(windows) {
        system_root().join("System32").join("tar.exe")
    } else {
        PathBuf::from("/usr/bin/tar")
    }
}

#[cfg(windows)]
fn powershell_path() -> PathBuf {
    system_root()
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe")
}

fn system_root() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
}

#[cfg(not(windows))]
fn script_command(script: &Path, home: &Path) -> Command {
    let mut c = crate::detect::command(Path::new("/bin/bash"));
    c.arg(script).env("HOME", home);
    c
}

#[cfg(windows)]
fn script_command(script: &Path, home: &Path) -> Command {
    let mut c = crate::detect::command(&powershell_path());
    c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .env("USERPROFILE", home)
        // 부모가 pwsh 7 등이면 PSModulePath가 오염되어 5.1이 기본 cmdlet을 못 찾는다.
        // 제거하면 PowerShell이 자기 기본 모듈 경로를 재구성한다.
        .env_remove("PSModulePath");
    c
}

/// macOS: ~/.local/bin이 셸 프로파일에 없으면 ~/.zshrc에 추가한다.
/// 반환값: 수정한 위치 (이미 설정돼 있으면 None)
#[cfg(not(windows))]
fn ensure_path(home: &Path) -> Result<Option<String>, AppError> {
    let profiles = [".zshrc", ".zprofile", ".bashrc", ".bash_profile"];
    for name in profiles {
        let p = home.join(name);
        if let Ok(content) = std::fs::read_to_string(&p) {
            if content.contains(".local/bin") {
                return Ok(None);
            }
        }
    }
    let zshrc = home.join(".zshrc");
    let snippet = "\n# Added by Hello, Agent so coding agents are available in the terminal\nexport PATH=\"$HOME/.local/bin:$PATH\"\n";
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&zshrc)
        .map_err(|e| AppError::classify(e.to_string()))?;
    f.write_all(snippet.as_bytes())
        .map_err(|e| AppError::classify(e.to_string()))?;
    Ok(Some(zshrc.display().to_string()))
}

/// Windows: 사용자 PATH(HKCU\Environment)에 %USERPROFILE%\.local\bin을 추가한다.
/// SetEnvironmentVariable('User')는 WM_SETTINGCHANGE 브로드캐스트까지 수행하므로
/// 새로 여는 터미널에 바로 반영된다.
#[cfg(windows)]
fn ensure_path(home: &Path) -> Result<Option<String>, AppError> {
    let bin_dir = home.join(".local").join("bin");
    let ps = format!(
        "$dir = '{}'; \
         $p = [Environment]::GetEnvironmentVariable('Path','User'); \
         if (($p -split ';') -contains $dir) {{ 'exists' }} \
         else {{ [Environment]::SetEnvironmentVariable('Path', ($dir + ';' + $p), 'User'); 'updated' }}",
        bin_dir.display()
    );
    let out = crate::detect::command(&powershell_path())
        .args(["-NoProfile", "-Command", &ps])
        .env_remove("PSModulePath")
        .output()
        .map_err(|e| AppError::classify(e.to_string()))?;
    if !out.status.success() {
        return Err(AppError::generic("failed to update user PATH via PowerShell"));
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if text == "updated" {
        Some("user PATH registry".into())
    } else {
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn isolated_install(agent: Agent, tag: &str) -> (PathBuf, InstallResult) {
        let home = std::env::temp_dir().join(format!(
            "hello-agent-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&home).unwrap();
        let events = Mutex::new(Vec::new());
        let result = run_install(agent, &home, &|e| {
            println!("{e:?}");
            events.lock().unwrap().push(e);
        });
        let report = result.expect("install should succeed");
        println!("{report:#?}");
        (home, report)
    }

    fn assert_common(home: &Path, report: &InstallResult, bin_name: &str) {
        let bin = home
            .join(".local")
            .join("bin")
            .join(crate::detect::exe(bin_name));
        assert!(bin.is_file());
        #[cfg(not(windows))]
        {
            let zshrc = std::fs::read_to_string(home.join(".zshrc")).unwrap();
            assert!(zshrc.contains(".local/bin"));
        }
        #[cfg(windows)]
        println!("windows profile_updated = {:?}", report.profile_updated);
        let _ = report;
    }

    /// 실행: cargo test -- --ignored --nocapture
    /// (Windows에서는 사용자 PATH 레지스트리에 임시 경로가 남는다 — CI 일회용 러너 전제)
    #[test]
    #[ignore = "network + ~1분 소요"]
    fn isolated_install_claude_end_to_end() {
        let (home, report) = isolated_install(Agent::ClaudeCode, "claude");
        assert!(report.version.contains("Claude Code"));
        assert_common(&home, &report, "claude");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    #[ignore = "network + ~1분 소요"]
    fn isolated_install_codex_end_to_end() {
        let (home, report) = isolated_install(Agent::Codex, "codex");
        assert!(report.version.contains("codex"));
        assert_common(&home, &report, "codex");
        std::fs::remove_dir_all(&home).ok();
    }
}
