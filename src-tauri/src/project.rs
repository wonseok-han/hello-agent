use crate::agent::Agent;
use crate::error::AppError;
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

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScannedProject {
    pub path: String,
    pub name: String,
    /// 표식으로 추정한 에이전트 ("claude-code" | "codex")
    pub agent: String,
}

/// 기본 프로젝트 폴더(사용자가 지정 안 하면 Documents).
#[tauri::command]
pub fn default_projects_dir() -> String {
    documents_dir().display().to_string()
}

/// 지정한 폴더의 바로 아래 하위 폴더 중, 에이전트 프로젝트 표식이 있는 것을 찾아 목록화한다.
/// 표식: `.claude/`·`CLAUDE.md`(클로드), `AGENTS.md`(코덱스). 앱 기억(store)과 무관하게
/// 디스크에서 실제 프로젝트를 발견하므로, 다른 데서 만든 프로젝트도 잡힌다.
#[tauri::command]
pub async fn scan_projects(base: String) -> Result<Vec<ScannedProject>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = PathBuf::from(&base);
        let mut found = Vec::new();
        let entries = std::fs::read_dir(&dir).map_err(|e| {
            AppError::classify(format!(
                "cannot read project base directory '{}': {e}",
                dir.display()
            ))
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(agent) = detect_project_agent(&path) {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                found.push(ScannedProject {
                    path: path.display().to_string(),
                    name,
                    agent: agent.into(),
                });
            }
        }
        found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(found)
    })
    .await
    .map_err(|e| AppError::generic(e.to_string()))?
}

/// 폴더의 표식으로 에이전트를 추정 (클로드 우선 — 둘 다 있으면 클로드).
fn detect_project_agent(dir: &Path) -> Option<&'static str> {
    if dir.join(".claude").is_dir() || dir.join("CLAUDE.md").is_file() {
        Some("claude-code")
    } else if dir.join("AGENTS.md").is_file() {
        Some("codex")
    } else {
        None
    }
}

/// 초보자 안내 프리셋: 프로젝트의 에이전트에게 사용자가 초보자임을 알린다.
const BEGINNER_GUIDE_KO: &str = "# 내 첫 프로젝트

이 폴더의 주인은 코딩을 처음 해 보는 사용자예요. 함께 일할 때:

- 쉬운 한국어로, 전문용어 없이 설명해 주세요.
- 파일을 지우거나 이 폴더 밖의 것을 바꾸기 전에는 반드시 먼저 물어봐 주세요.
- 작업을 마치면 무엇을 했는지 쉬운 말로 한 줄 정리해 주세요.
";

const BEGINNER_GUIDE_EN: &str = "# My first project

The owner of this folder is new to coding. When working together:

- Explain things in plain English without jargon.
- Ask before deleting files or changing anything outside this folder.
- When a task is complete, summarize what you changed in one simple sentence.
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
const CODEX_SAFE_CONFIG_KO: &str = "# Hello, Agent가 만든 초보자 안전 설정
approval_policy = \"untrusted\"
sandbox_mode = \"workspace-write\"
";

const CODEX_SAFE_CONFIG_EN: &str = "# Beginner-safe defaults created by Hello, Agent
approval_policy = \"untrusted\"
sandbox_mode = \"workspace-write\"
";

#[tauri::command]
pub async fn create_first_project(
    agent: String,
    name: Option<String>,
    base: Option<String>,
    language: Option<String>,
) -> Result<ProjectInfo, AppError> {
    let agent = Agent::from_id(&agent)?;
    tauri::async_runtime::spawn_blocking(move || {
        create(agent, name, base, language).map_err(AppError::classify)
    })
    .await
    .map_err(|e| AppError::generic(e.to_string()))?
}

fn create(
    agent: Agent,
    name: Option<String>,
    base: Option<String>,
    language: Option<String>,
) -> Result<ProjectInfo, String> {
    let name = sanitize_name(name.as_deref().unwrap_or("my-first-project"));
    if name.is_empty() {
        return Err("project folder name has no usable characters".into());
    }
    let base_dir = base.map(PathBuf::from).unwrap_or_else(documents_dir);
    let dir = base_dir.join(&name);
    let created = !dir.exists();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create project folder '{}': {e}", dir.display()))?;

    apply_safety_preset(agent, &dir, language.as_deref() == Some("en"))?;

    Ok(ProjectInfo {
        path: dir.display().to_string(),
        created,
    })
}

/// 에이전트별 안전 프리셋. 이미 있는 파일은 절대 덮어쓰지 않는다.
fn apply_safety_preset(agent: Agent, dir: &Path, english: bool) -> Result<(), String> {
    let write_if_absent = |path: PathBuf, content: &str| -> Result<(), String> {
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!("cannot create settings folder '{}': {e}", parent.display())
            })?;
        }
        std::fs::write(&path, content)
            .map_err(|e| format!("cannot write settings file '{}': {e}", path.display()))
    };

    match agent {
        Agent::ClaudeCode => {
            let guide = if english {
                BEGINNER_GUIDE_EN
            } else {
                BEGINNER_GUIDE_KO
            };
            write_if_absent(dir.join("CLAUDE.md"), guide)?;
            write_if_absent(
                dir.join(".claude").join("settings.json"),
                CLAUDE_SAFE_SETTINGS,
            )?;
        }
        Agent::Codex => {
            let guide = if english {
                BEGINNER_GUIDE_EN
            } else {
                BEGINNER_GUIDE_KO
            };
            let config = if english {
                CODEX_SAFE_CONFIG_EN
            } else {
                CODEX_SAFE_CONFIG_KO
            };
            write_if_absent(dir.join("AGENTS.md"), guide)?;
            // 코덱스의 승인 정책은 전역 설정이므로, 설정 파일이 아예 없을 때만 생성
            write_if_absent(
                crate::detect::home_dir().join(".codex").join("config.toml"),
                config,
            )?;
        }
    }
    Ok(())
}

/// 프로젝트 폴더에서 비대화형으로 첫 인사를 주고받는다.
#[tauri::command]
pub async fn run_first_chat(
    agent: String,
    project_path: String,
    language: Option<String>,
) -> Result<String, AppError> {
    let agent = Agent::from_id(&agent)?;
    tauri::async_runtime::spawn_blocking(move || {
        let dir = PathBuf::from(&project_path);
        if !dir.is_dir() {
            return Err(AppError::not_found("project folder not found"));
        }
        let bin = crate::detect::agent_bin(agent)
            .ok_or_else(|| AppError::not_found(format!("{} is not installed", agent.bin_name())))?;
        let prompt = first_chat_prompt(language.as_deref());
        let out = crate::detect::command(&bin)
            .args(agent.chat_args(prompt))
            .current_dir(&dir)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| AppError::classify(e.to_string()))?;
        let reply = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !reply.is_empty() {
            Ok(clean_reply(agent, &reply))
        } else {
            // 에이전트 stderr에 원인이 담기므로 그대로 분류(네트워크 등)
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(AppError::classify(if err.is_empty() {
                "agent returned no reply".to_string()
            } else {
                err
            }))
        }
    })
    .await
    .map_err(|e| AppError::generic(e.to_string()))?
}

fn first_chat_prompt(language: Option<&str>) -> &'static str {
    if language == Some("en") {
        "Give a new coding-agent user a short, warm welcome in plain English using no more than two sentences."
    } else {
        "코딩 도우미를 처음 만나는 사용자에게 두 문장 이내의 짧고 따뜻한 한국어 환영 인사를 해 주세요."
    }
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
        let base = std::env::temp_dir().join(format!("ha-base-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let name = format!("hello-agent-테스트-{}", std::process::id());
        let base_s = Some(base.display().to_string());
        let info = create(
            Agent::ClaudeCode,
            Some(name.clone()),
            base_s.clone(),
            Some("ko".into()),
        )
        .unwrap();
        let dir = std::path::PathBuf::from(&info.path);
        assert!(info.created);
        assert!(dir.starts_with(&base));
        assert!(dir.join("CLAUDE.md").is_file());
        let settings = std::fs::read_to_string(dir.join(".claude").join("settings.json")).unwrap();
        assert!(settings.contains("deny"));
        serde_json::from_str::<serde_json::Value>(&settings).expect("valid json");
        // 두 번째 호출은 재사용으로 판정돼야 함
        let again = create(Agent::ClaudeCode, Some(name), base_s, Some("ko".into())).unwrap();
        assert!(!again.created);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn create_project_localizes_beginner_guide() {
        let base = std::env::temp_dir().join(format!("ha-en-base-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let info = create(
            Agent::ClaudeCode,
            Some("english-project".into()),
            Some(base.display().to_string()),
            Some("en".into()),
        )
        .unwrap();
        let guide = std::fs::read_to_string(PathBuf::from(info.path).join("CLAUDE.md")).unwrap();
        assert!(guide.contains("plain English"));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn scan_finds_projects_by_marker() {
        let base = std::env::temp_dir().join(format!("ha-scan-{}", std::process::id()));
        // 클로드 표식 폴더
        std::fs::create_dir_all(base.join("proj-claude")).unwrap();
        std::fs::write(base.join("proj-claude").join("CLAUDE.md"), "x").unwrap();
        // 코덱스 표식 폴더
        std::fs::create_dir_all(base.join("proj-codex")).unwrap();
        std::fs::write(base.join("proj-codex").join("AGENTS.md"), "x").unwrap();
        // 표식 없는 폴더 — 제외돼야 함
        std::fs::create_dir_all(base.join("just-a-folder")).unwrap();

        let dir = base.clone();
        let mut found: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| super::detect_project_agent(&e.path()).map(|a| (e.file_name(), a)))
            .collect();
        found.sort_by_key(|(n, _)| n.to_string_lossy().to_string());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1, "claude-code");
        assert_eq!(found[1].1, "codex");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn scan_reports_missing_base_directory() {
        let missing = std::env::temp_dir().join(format!("ha-missing-{}", std::process::id()));
        std::fs::remove_dir_all(&missing).ok();
        let error = tauri::async_runtime::block_on(scan_projects(missing.display().to_string()))
            .expect_err("missing base directory should be reported");
        assert_eq!(error.kind, crate::error::ErrorKind::NotFound);
    }

    #[test]
    fn codex_reply_cleanup_keeps_last_block() {
        let raw = "[2026-07-16] OpenAI Codex v0.144.5\nworkdir: /tmp\n\ntokens used: 123\n\n안녕하세요! 만나서 반가워요.";
        assert_eq!(
            clean_reply(Agent::Codex, raw),
            "안녕하세요! 만나서 반가워요."
        );
    }

    #[test]
    fn first_chat_prompt_follows_language() {
        assert!(first_chat_prompt(Some("ko")).contains("한국어"));
        assert!(first_chat_prompt(Some("en")).contains("English"));
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
            Some("ko".into()),
        ))
        .unwrap();
        println!("응답: {reply}");
        assert!(!reply.is_empty());
        std::fs::remove_dir_all(&home).ok();
    }
}
