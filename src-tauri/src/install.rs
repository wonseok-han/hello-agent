use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::ipc::Channel;

const SCRIPT_URL_UNIX: &str = "https://claude.ai/install.sh";
const SCRIPT_URL_WINDOWS: &str = "https://claude.ai/install.ps1";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "type")]
pub enum InstallEvent {
    Phase { name: String },
    Log { line: String },
    /// 지금까지 내려받은 바이트 수 (다운로드 폴더 크기 관찰값)
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
/// M1에서 UI 노출 없이 내부 테스트로만 쓰도록 정리 예정.
#[tauri::command]
pub async fn install_claude_code(
    test_home: Option<String>,
    on_event: Channel<InstallEvent>,
) -> Result<InstallResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let home = match test_home {
            Some(h) => PathBuf::from(h),
            None => crate::detect::home_dir(),
        };
        run_install(&home, &|e| {
            let _ = on_event.send(e);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn run_install(
    home: &Path,
    emit: &(dyn Fn(InstallEvent) + Sync),
) -> Result<InstallResult, String> {
    let phase = |name: &str| emit(InstallEvent::Phase { name: name.into() });

    // 1. 공식 설치 스크립트 다운로드 (curl은 macOS·Windows 10+ 모두 기본 탑재)
    phase("download");
    let script_dir = home.join(".claude");
    std::fs::create_dir_all(&script_dir)
        .map_err(|e| format!("설치 준비 폴더를 만들지 못했어요: {e}"))?;
    let (url, script_name) = if cfg!(windows) {
        (SCRIPT_URL_WINDOWS, "agent-starter-install.ps1")
    } else {
        (SCRIPT_URL_UNIX, "agent-starter-install.sh")
    };
    let script = script_dir.join(script_name);
    let status = crate::detect::command(&curl_path())
        .args(["-fsSL", url, "-o"])
        .arg(&script)
        .status()
        .map_err(|e| format!("다운로드 도구를 실행하지 못했어요: {e}"))?;
    if !status.success() {
        return Err("설치 파일을 내려받지 못했어요. 인터넷 연결을 확인해 주세요.".into());
    }

    // 2. 인스톨러 무인 실행 — 출력을 실시간으로 흘려보냄
    phase("install");
    let mut child = installer_command(&script, home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("설치를 시작하지 못했어요: {e}"))?;

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    // 인스톨러는 다운로드 진행률을 출력하지 않으므로,
    // 다운로드 폴더 크기를 관찰해 진행 이벤트를 만든다
    let downloads_dir = home.join(".claude").join("downloads");
    let stop = std::sync::atomic::AtomicBool::new(false);
    let status = std::thread::scope(|s| {
        for reader in [
            Box::new(stdout) as Box<dyn std::io::Read + Send>,
            Box::new(stderr),
        ] {
            s.spawn(move || {
                for line in BufReader::new(reader).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        emit(InstallEvent::Log { line });
                    }
                }
            });
        }
        let (stop_ref, dir) = (&stop, downloads_dir);
        s.spawn(move || {
            let mut last = 0u64;
            while !stop_ref.load(std::sync::atomic::Ordering::Relaxed) {
                let size = dir_size(&dir);
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
    .map_err(|e| format!("설치 진행 상태를 확인하지 못했어요: {e}"))?;
    let _ = std::fs::remove_file(&script);
    if !status.success() {
        return Err(format!(
            "설치가 중간에 멈췄어요 (코드 {}). 인터넷 연결을 확인하고 다시 시도해 주세요.",
            status.code().unwrap_or(-1)
        ));
    }

    // 3. 터미널 PATH 설정 — 인스톨러가 안 해 주는 경우를 대비해 직접 반영 (docs §5)
    phase("path");
    let profile_updated = ensure_path(home)?;

    // 4. 절대경로로 설치 검증 — PATH에 의존하지 않음
    phase("verify");
    let bin = home
        .join(".local")
        .join("bin")
        .join(crate::detect::exe("claude"));
    let out = crate::detect::command(&bin)
        .arg("--version")
        .env(home_env_var(), home)
        .output()
        .map_err(|e| format!("설치된 클로드 코드를 실행하지 못했어요: {e}"))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || version.is_empty() {
        return Err("설치는 끝났지만 동작 확인에 실패했어요.".into());
    }

    Ok(InstallResult {
        version,
        path: bin.display().to_string(),
        profile_updated,
    })
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().and_then(|e| e.metadata().ok()))
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
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
fn installer_command(script: &Path, home: &Path) -> Command {
    let mut c = crate::detect::command(Path::new("/bin/bash"));
    c.arg(script).env("HOME", home);
    c
}

#[cfg(windows)]
fn installer_command(script: &Path, home: &Path) -> Command {
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
fn ensure_path(home: &Path) -> Result<Option<String>, String> {
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
    let snippet = "\n# agent-starter가 추가함: 터미널에서 claude 명령을 찾을 수 있게 하는 설정\nexport PATH=\"$HOME/.local/bin:$PATH\"\n";
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&zshrc)
        .map_err(|e| format!("터미널 설정 파일을 열지 못했어요: {e}"))?;
    f.write_all(snippet.as_bytes())
        .map_err(|e| format!("터미널 설정을 저장하지 못했어요: {e}"))?;
    Ok(Some(zshrc.display().to_string()))
}

/// Windows: 사용자 PATH(HKCU\Environment)에 %USERPROFILE%\.local\bin을 추가한다.
/// SetEnvironmentVariable('User')는 WM_SETTINGCHANGE 브로드캐스트까지 수행하므로
/// 새로 여는 터미널에 바로 반영된다.
#[cfg(windows)]
fn ensure_path(home: &Path) -> Result<Option<String>, String> {
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
        .map_err(|e| format!("터미널 설정 도구를 실행하지 못했어요: {e}"))?;
    if !out.status.success() {
        return Err("터미널 설정을 저장하지 못했어요.".into());
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if text == "updated" {
        Some("사용자 PATH (레지스트리)".into())
    } else {
        None
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    /// 네트워크로 실제 인스톨러를 내려받아 임시 HOME에 격리 설치한다.
    /// 실행: cargo test -- --ignored --nocapture
    /// (Windows에서는 사용자 PATH 레지스트리에 임시 경로가 남는다 — CI 일회용 러너 전제)
    #[test]
    #[ignore = "network + ~1분 소요"]
    fn isolated_install_end_to_end() {
        let home = std::env::temp_dir().join(format!("agent-starter-test-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();

        let events = Mutex::new(Vec::new());
        let result = super::run_install(&home, &|e| {
            println!("{e:?}");
            events.lock().unwrap().push(e);
        });

        let report = result.expect("install should succeed");
        println!("{report:#?}");
        assert!(report.version.contains("Claude Code"));
        let bin = home
            .join(".local")
            .join("bin")
            .join(crate::detect::exe("claude"));
        assert!(bin.is_file());

        #[cfg(not(windows))]
        {
            // 빈 HOME에는 프로파일이 없으므로 .zshrc가 생성돼야 함
            let zshrc = std::fs::read_to_string(home.join(".zshrc")).unwrap();
            assert!(zshrc.contains(".local/bin"));
            assert_eq!(
                report.profile_updated,
                Some(home.join(".zshrc").display().to_string())
            );
        }
        #[cfg(windows)]
        {
            // 인스톨러가 PATH를 직접 처리하는지는 이 값으로 관찰한다 (None이면 인스톨러가 이미 등록한 것)
            println!("windows profile_updated = {:?}", report.profile_updated);
        }

        std::fs::remove_dir_all(&home).ok();
    }
}
