use crate::agent::Agent;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Stdio};
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::State;

/// 로그인 상태 메타데이터 (비밀값 없음)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus {
    pub logged_in: bool,
    #[serde(default)]
    pub auth_method: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub subscription_type: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum LoginEvent {
    /// 브라우저에서 열어야 할 로그인 URL
    Url { url: String },
    Log { line: String },
    /// 로그인 프로세스 종료 (성공 여부는 상태 재확인으로 판정)
    Exit { success: bool },
}

#[derive(Default)]
pub struct LoginSession(pub Mutex<Option<Child>>);

#[tauri::command]
pub async fn login_status(agent: String) -> Result<LoginStatus, AppError> {
    let agent = Agent::from_id(&agent)?;
    tauri::async_runtime::spawn_blocking(move || {
        let bin = crate::detect::agent_bin(agent)
            .ok_or_else(|| AppError::not_found(format!("{} is not installed", agent.bin_name())))?;
        query_status(agent, &bin).map_err(AppError::classify)
    })
    .await
    .map_err(|e| AppError::generic(e.to_string()))?
}

fn query_status(agent: Agent, bin: &std::path::Path) -> Result<LoginStatus, String> {
    let out = crate::detect::command(bin)
        .args(agent.status_args())
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot check sign-in status: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();

    match agent {
        Agent::ClaudeCode => serde_json::from_str(&text)
            .map_err(|e| format!("cannot parse Claude sign-in status response: {e}")),
        // codex는 JSON이 없으므로 종료 코드와 텍스트로 판정
        Agent::Codex => Ok(LoginStatus {
            logged_in: out.status.success(),
            auth_method: out
                .status
                .success()
                .then(|| {
                    if text.to_lowercase().contains("api key") {
                        "api-key".to_string()
                    } else {
                        "chatgpt".to_string()
                    }
                }),
            email: None,
            subscription_type: None,
        }),
    }
}

pub(crate) fn is_logged_in(agent: Agent) -> bool {
    crate::detect::agent_bin(agent)
        .and_then(|bin| query_status(agent, &bin).ok())
        .map(|s| s.logged_in)
        .unwrap_or(false)
}

/// 로그인을 백그라운드로 시작한다. CLI가 브라우저를 직접 열고, 대비용 URL도 이벤트로 전달.
/// 완료 감지는 프론트의 상태 폴링이 담당한다.
/// `use_api_billing`: 클로드의 콘솔(API 과금) 로그인 분기 (M2 요금제 안내)
#[tauri::command]
pub fn start_login(
    agent: String,
    use_api_billing: Option<bool>,
    session: State<'_, LoginSession>,
    on_event: Channel<LoginEvent>,
) -> Result<(), AppError> {
    let agent = Agent::from_id(&agent)?;
    let bin = crate::detect::agent_bin(agent)
        .ok_or_else(|| AppError::not_found(format!("{} is not installed", agent.bin_name())))?;

    let mut guard = session.0.lock().unwrap();
    if let Some(mut old) = guard.take() {
        let _ = old.kill();
        let _ = old.wait();
    }

    let mut child = crate::detect::command(&bin)
        .args(agent.login_args(use_api_billing.unwrap_or(false)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::classify(e.to_string()))?;

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    *guard = Some(child);
    drop(guard);

    for reader in [
        Box::new(stdout) as Box<dyn std::io::Read + Send>,
        Box::new(stderr),
    ] {
        let ch = on_event.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                if let Some(url) = extract_url(&line) {
                    let _ = ch.send(LoginEvent::Url { url });
                } else if !line.trim().is_empty() {
                    let _ = ch.send(LoginEvent::Log { line });
                }
            }
        });
    }
    Ok(())
}

/// 브라우저 로그인 후 받은 확인 코드를 CLI에 전달하고 종료를 기다린다 (클로드 폴백 경로).
#[tauri::command]
pub async fn submit_login_code(
    agent: String,
    session: State<'_, LoginSession>,
    on_event: Channel<LoginEvent>,
    code: String,
) -> Result<(), AppError> {
    let agent = Agent::from_id(&agent)?;
    let mut child = session
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| AppError::generic("no login in progress"))?;

    tauri::async_runtime::spawn_blocking(move || {
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(format!("{}\n", code.trim()).as_bytes())
                .map_err(|e| AppError::classify(e.to_string()))?;
        }
        // 프로세스 종료를 기다리되, 코드 교환이 성공했는데도 CLI가 안 끝나는 경우
        // (조직 선택 프롬프트 등)가 있으므로 로그인 상태를 함께 폴링해 판정한다
        let mut success = false;
        let start = std::time::Instant::now();
        let mut last_status_check = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    success = status.success();
                    break;
                }
                Ok(None) => {}
                Err(_) => break,
            }
            if last_status_check.elapsed() > std::time::Duration::from_secs(3) {
                last_status_check = std::time::Instant::now();
                if is_logged_in(agent) {
                    success = true;
                    break;
                }
            }
            if start.elapsed() > std::time::Duration::from_secs(60) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // 최종 안전망: 프로세스 판정과 무관하게 실제 로그인 여부가 진실
        if !success {
            success = is_logged_in(agent);
        }
        let _ = on_event.send(LoginEvent::Exit { success });
        if success {
            Ok(())
        } else {
            Err(AppError::generic("login code verification failed"))
        }
    })
    .await
    .map_err(|e| AppError::generic(e.to_string()))?
}

#[tauri::command]
pub fn cancel_login(session: State<'_, LoginSession>) {
    if let Some(mut child) = session.0.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// 터미널 하이퍼링크 이스케이프(OSC 8)가 섞인 줄에서 로그인 URL만 뽑아낸다
fn extract_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '\x1b' || c == '\x07')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_url_from_osc8_line() {
        let line = "If the browser didn't open, visit: \x1b]8;;https://claude.com/cai/oauth/authorize?code=true&state=abc\x1b\\https://claude.com/cai/oauth/authorize?code=true&state=abc\x1b]8;;\x1b\\";
        let url = extract_url(line).unwrap();
        assert_eq!(
            url,
            "https://claude.com/cai/oauth/authorize?code=true&state=abc"
        );
    }

    #[test]
    fn parse_claude_status_json() {
        let json = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"a@b.c","orgId":"x","orgName":"y","subscriptionType":"pro"}"#;
        let s: LoginStatus = serde_json::from_str(json).unwrap();
        assert!(s.logged_in);
        assert_eq!(s.subscription_type.as_deref(), Some("pro"));
    }

    /// 실기기 상태 조회 (읽기 전용). 실행: cargo test -- --ignored
    #[test]
    #[ignore = "claude 설치 필요"]
    fn login_status_on_this_machine() {
        let bin = crate::detect::agent_bin(Agent::ClaudeCode).expect("claude installed");
        let s = query_status(Agent::ClaudeCode, &bin).unwrap();
        println!("loggedIn={} type={:?}", s.logged_in, s.subscription_type);
    }
}
