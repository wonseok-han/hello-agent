use crate::agent::Agent;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub path: String,
    /// false면 이미 있던 폴더를 그대로 재사용
    pub created: bool,
}

/// 초보자 안내 프리셋: 프로젝트의 에이전트에게 사용자가 초보자임을 알린다.
const BEGINNER_GUIDE_MD: &str = "# 내 첫 프로젝트

이 폴더의 주인은 코딩을 처음 해 보는 사용자예요. 함께 일할 때:

- 쉬운 한국어로, 전문용어 없이 설명해 주세요.
- 파일을 지우거나 이 폴더 밖의 것을 바꾸기 전에는 반드시 먼저 물어봐 주세요.
- 작업을 마치면 무엇을 했는지 쉬운 말로 한 줄 정리해 주세요.
";

/// 클로드 코드 프로젝트 안전 프리셋 — 민감 파일 읽기와 위험 명령을 기본 차단
/// (초보자의 "11GB 삭제 사건" 류 사고 방지, docs/architecture.md §6)
const CLAUDE_SAFE_SETTINGS: &str = r#"{
  "permissions": {
    "deny": [
      "Read(**/.env)",
      "Read(**/.env.*)",
      "Read(~/.ssh/**)",
      "Bash(rm -rf:*)"
    ]
  }
}
"#;

/// 코덱스 전역 안전 프리셋 — 확인 없는 명령 실행 금지 + 작업 폴더 밖 쓰기 차단.
/// 사용자가 이미 설정 파일을 갖고 있으면 절대 건드리지 않는다.
const CODEX_SAFE_CONFIG: &str = "# Hello, Agent가 만든 초보자 안전 설정
approval_policy = \"untrusted\"
sandbox_mode = \"workspace-write\"
";

#[tauri::command]
pub async fn create_first_project(
    agent: String,
    name: Option<String>,
) -> Result<ProjectInfo, String> {
    let agent = Agent::from_id(&agent)?;
    tauri::async_runtime::spawn_blocking(move || create(agent, name))
        .await
        .map_err(|e| e.to_string())?
}

fn create(agent: Agent, name: Option<String>) -> Result<ProjectInfo, String> {
    let name = sanitize_name(name.as_deref().unwrap_or("my-first-project"));
    if name.is_empty() {
        return Err("폴더 이름에 쓸 수 있는 글자가 없어요. 다른 이름을 지어 주세요.".into());
    }
    let dir = documents_dir().join(&name);
    let created = !dir.exists();
    std::fs::create_dir_all(&dir).map_err(|e| format!("폴더를 만들지 못했어요: {e}"))?;

    apply_safety_preset(agent, &dir)?;

    Ok(ProjectInfo {
        path: dir.display().to_string(),
        created,
    })
}

/// 에이전트별 안전 프리셋. 이미 있는 파일은 절대 덮어쓰지 않는다.
fn apply_safety_preset(agent: Agent, dir: &Path) -> Result<(), String> {
    let write_if_absent = |path: PathBuf, content: &str| -> Result<(), String> {
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("설정 폴더를 만들지 못했어요: {e}"))?;
        }
        std::fs::write(&path, content).map_err(|e| format!("설정 파일을 만들지 못했어요: {e}"))
    };

    match agent {
        Agent::ClaudeCode => {
            write_if_absent(dir.join("CLAUDE.md"), BEGINNER_GUIDE_MD)?;
            write_if_absent(dir.join(".claude").join("settings.json"), CLAUDE_SAFE_SETTINGS)?;
        }
        Agent::Codex => {
            write_if_absent(dir.join("AGENTS.md"), BEGINNER_GUIDE_MD)?;
            // 코덱스의 승인 정책은 전역 설정이므로, 설정 파일이 아예 없을 때만 생성
            write_if_absent(
                crate::detect::home_dir().join(".codex").join("config.toml"),
                CODEX_SAFE_CONFIG,
            )?;
        }
    }
    Ok(())
}

/// 프로젝트 폴더에서 비대화형으로 첫 인사를 주고받는다.
#[tauri::command]
pub async fn run_first_chat(agent: String, project_path: String) -> Result<String, String> {
    let agent = Agent::from_id(&agent)?;
    tauri::async_runtime::spawn_blocking(move || {
        let dir = PathBuf::from(&project_path);
        if !dir.is_dir() {
            return Err("프로젝트 폴더를 찾을 수 없어요. 이전 단계로 돌아가 주세요.".into());
        }
        let bin = crate::detect::agent_bin(agent).ok_or_else(|| {
            format!("{}가 아직 설치되어 있지 않아요.", agent.display_name())
        })?;
        let prompt =
            "코딩 도우미를 처음 만나는 사용자에게 두 문장 이내의 짧고 따뜻한 한국어 환영 인사를 해 주세요.";
        let out = crate::detect::command(&bin)
            .args(agent.chat_args(prompt))
            .current_dir(&dir)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("{}를 실행하지 못했어요: {e}", agent.display_name()))?;
        let reply = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !reply.is_empty() {
            Ok(clean_reply(agent, &reply))
        } else {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if err.is_empty() {
                "대답을 받지 못했어요. 다시 시도해 주세요.".into()
            } else {
                format!("대답을 받지 못했어요: {err}")
            })
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// codex exec는 응답 앞에 메타 정보 블록을 출력하므로 마지막 문단만 남긴다
fn clean_reply(agent: Agent, raw: &str) -> String {
    match agent {
        Agent::ClaudeCode => raw.to_string(),
        Agent::Codex => raw
            .split("\n\n")
            .filter(|block| !block.trim().is_empty())
            .last()
            .unwrap_or(raw)
            .trim()
            .to_string(),
    }
}

fn documents_dir() -> PathBuf {
    crate::detect::home_dir().join("Documents")
}

/// 경로 구분자·제어문자를 제거하고 앞뒤 공백과 점을 정리한다
fn sanitize_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| {
            !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
        })
        .collect();
    cleaned.trim().trim_matches('.').trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_path_separators() {
        assert_eq!(sanitize_name("../../../etc"), "etc");
        assert_eq!(sanitize_name("내 프로젝트"), "내 프로젝트");
        assert_eq!(sanitize_name("a/b\\c:d"), "abcd");
        assert_eq!(sanitize_name("///"), "");
    }

    #[test]
    fn create_claude_project_with_safety_preset() {
        let name = format!("hello-agent-테스트-{}", std::process::id());
        let info = create(Agent::ClaudeCode, Some(name.clone())).unwrap();
        let dir = std::path::PathBuf::from(&info.path);
        assert!(info.created);
        assert!(dir.join("CLAUDE.md").is_file());
        let settings =
            std::fs::read_to_string(dir.join(".claude").join("settings.json")).unwrap();
        assert!(settings.contains("deny"));
        serde_json::from_str::<serde_json::Value>(&settings).expect("valid json");
        // 두 번째 호출은 재사용으로 판정돼야 함
        let again = create(Agent::ClaudeCode, Some(name)).unwrap();
        assert!(!again.created);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn codex_reply_cleanup_keeps_last_block() {
        let raw = "[2026-07-16] OpenAI Codex v0.144.5\nworkdir: /tmp\n\ntokens used: 123\n\n안녕하세요! 만나서 반가워요.";
        assert_eq!(
            clean_reply(Agent::Codex, raw),
            "안녕하세요! 만나서 반가워요."
        );
    }

    /// 실제 첫 대화 실행 (사용량 소모). 실행: cargo test -- --ignored --nocapture
    #[test]
    #[ignore = "claude 로그인 + 사용량 소모"]
    fn first_chat_on_this_machine() {
        let home = std::env::temp_dir().join(format!("hello-agent-chat-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        let reply = tauri::async_runtime::block_on(run_first_chat(
            "claude-code".into(),
            home.display().to_string(),
        ))
        .unwrap();
        println!("응답: {reply}");
        assert!(!reply.is_empty());
        std::fs::remove_dir_all(&home).ok();
    }
}
