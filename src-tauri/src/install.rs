use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::ipc::Channel;

const INSTALL_SCRIPT_URL: &str = "https://claude.ai/install.sh";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum InstallEvent {
    Phase { name: String },
    Log { line: String },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub version: String,
    pub path: String,
    /// PATH 설정을 추가한 프로파일 파일 (이미 설정돼 있었으면 None)
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
    if !cfg!(target_os = "macos") {
        return Err("아직 macOS에서만 설치를 지원해요. Windows는 준비 중이에요.".into());
    }
    let phase = |name: &str| emit(InstallEvent::Phase { name: name.into() });

    // 1. 공식 설치 스크립트 다운로드
    phase("download");
    let script_dir = home.join(".claude");
    std::fs::create_dir_all(&script_dir)
        .map_err(|e| format!("설치 준비 폴더를 만들지 못했어요: {e}"))?;
    let script = script_dir.join("agent-starter-install.sh");
    let status = Command::new("/usr/bin/curl")
        .args(["-fsSL", INSTALL_SCRIPT_URL, "-o"])
        .arg(&script)
        .status()
        .map_err(|e| format!("다운로드 도구를 실행하지 못했어요: {e}"))?;
    if !status.success() {
        return Err("설치 파일을 내려받지 못했어요. 인터넷 연결을 확인해 주세요.".into());
    }

    // 2. 인스톨러 무인 실행 — 출력을 실시간으로 흘려보냄
    phase("install");
    let mut child = Command::new("/bin/bash")
        .arg(&script)
        .env("HOME", home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("설치를 시작하지 못했어요: {e}"))?;

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    std::thread::scope(|s| {
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
    });
    let status = child
        .wait()
        .map_err(|e| format!("설치 진행 상태를 확인하지 못했어요: {e}"))?;
    let _ = std::fs::remove_file(&script);
    if !status.success() {
        return Err(format!(
            "설치가 중간에 멈췄어요 (코드 {}). 다시 시도해 주세요.",
            status.code().unwrap_or(-1)
        ));
    }

    // 3. 터미널 PATH 설정 — 인스톨러는 안내문만 출력하므로 직접 반영 (docs/architecture.md §5)
    phase("path");
    let profile_updated = ensure_path(home)?;

    // 4. 절대경로로 설치 검증 — PATH에 의존하지 않음
    phase("verify");
    let bin = home.join(".local/bin/claude");
    let out = Command::new(&bin)
        .arg("--version")
        .env("HOME", home)
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

/// ~/.local/bin이 셸 프로파일에 없으면 ~/.zshrc에 추가한다.
/// 반환값: 수정한 파일 경로 (이미 설정돼 있으면 None)
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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    /// 네트워크로 실제 인스톨러를 내려받아 임시 HOME에 격리 설치한다.
    /// 실행: cargo test -- --ignored --nocapture
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
        assert!(home.join(".local/bin/claude").is_file());
        // 빈 HOME에는 프로파일이 없으므로 .zshrc가 생성돼야 함
        let zshrc = std::fs::read_to_string(home.join(".zshrc")).unwrap();
        assert!(zshrc.contains(".local/bin"));
        assert_eq!(report.profile_updated, Some(home.join(".zshrc").display().to_string()));

        std::fs::remove_dir_all(&home).ok();
    }
}
