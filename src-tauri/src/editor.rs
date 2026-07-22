use serde::Serialize;
use std::path::{Path, PathBuf};

/// 초보자에게 추천하는 코드 편집기. 설치 여부를 감지하고, 설치돼 있으면
/// 프로젝트 폴더를 그 편집기로 바로 열어준다 ("편집기에서 폴더 열기"를 대신).
#[derive(Clone, Copy)]
enum Editor {
    Cursor,
    VsCode,
}

impl Editor {
    const ALL: [Editor; 2] = [Editor::Cursor, Editor::VsCode];

    fn id(self) -> &'static str {
        match self {
            Editor::Cursor => "cursor",
            Editor::VsCode => "vscode",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Editor::Cursor => "커서(Cursor)",
            Editor::VsCode => "VS Code",
        }
    }

    fn url(self) -> &'static str {
        match self {
            Editor::Cursor => "https://cursor.com",
            Editor::VsCode => "https://code.visualstudio.com",
        }
    }

    fn from_id(id: &str) -> Option<Editor> {
        Editor::ALL.into_iter().find(|e| e.id() == id)
    }

    #[cfg(target_os = "macos")]
    fn mac_app_name(self) -> &'static str {
        match self {
            Editor::Cursor => "Cursor",
            Editor::VsCode => "Visual Studio Code",
        }
    }

    /// 확장 설치·폴더 열기에 쓰는 편집기 CLI 이름
    fn cli_name(self) -> &'static str {
        match self {
            Editor::Cursor => "cursor",
            Editor::VsCode => "code",
        }
    }

    /// 편집기 CLI(`--install-extension` 지원)의 절대경로. 확장 자동 설치용.
    fn cli_path(self, home: &Path) -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            let bin = format!("Contents/Resources/app/bin/{}", self.cli_name());
            self.install_paths(home)
                .into_iter()
                .map(|app| app.join(&bin))
                .find(|p| p.exists())
        }
        #[cfg(windows)]
        {
            let local = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_default();
            let pf = std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .unwrap_or_default();
            let _ = home;
            let cands: Vec<PathBuf> = match self {
                Editor::Cursor => {
                    vec![local.join(r"Programs\cursor\resources\app\bin\cursor.cmd")]
                }
                Editor::VsCode => vec![
                    local.join(r"Programs\Microsoft VS Code\bin\code.cmd"),
                    pf.join(r"Microsoft VS Code\bin\code.cmd"),
                ],
            };
            cands.into_iter().find(|p| p.exists())
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = home;
            None
        }
    }

    /// 설치 여부를 판단할 후보 경로들
    fn install_paths(self, home: &Path) -> Vec<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            let app = format!("{}.app", self.mac_app_name());
            vec![
                PathBuf::from("/Applications").join(&app),
                home.join("Applications").join(&app),
            ]
        }
        #[cfg(windows)]
        {
            let local = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_default();
            let pf = std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .unwrap_or_default();
            let _ = home;
            match self {
                Editor::Cursor => vec![
                    local.join(r"Programs\cursor\Cursor.exe"),
                    home.join(r"AppData\Local\Programs\cursor\Cursor.exe"),
                ],
                Editor::VsCode => vec![
                    local.join(r"Programs\Microsoft VS Code\Code.exe"),
                    pf.join(r"Microsoft VS Code\Code.exe"),
                ],
            }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = home;
            Vec::new()
        }
    }

    fn installed_path(self, home: &Path) -> Option<PathBuf> {
        self.install_paths(home).into_iter().find(|p| p.exists())
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EditorInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub installed: bool,
}

#[tauri::command]
pub async fn detect_editors() -> Vec<EditorInfo> {
    tauri::async_runtime::spawn_blocking(|| {
        let home = crate::detect::home_dir();
        Editor::ALL
            .into_iter()
            .map(|e| EditorInfo {
                id: e.id().into(),
                name: e.name().into(),
                url: e.url().into(),
                installed: e.installed_path(&home).is_some(),
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// 설치된 편집기로 프로젝트 폴더를 연다. 먼저 선택한 에이전트의 확장을
/// 자동 설치(이미 있으면 건너뜀)해 편집기 안에서 GUI로 바로 쓰게 한다.
#[tauri::command]
pub async fn open_in_editor(
    editor: String,
    agent: String,
    path: String,
) -> Result<(), crate::error::AppError> {
    use crate::error::AppError;
    let editor = Editor::from_id(&editor).ok_or_else(|| AppError::generic("unknown editor"))?;
    let ext = crate::agent::Agent::from_id(&agent)?.extension_id();
    tauri::async_runtime::spawn_blocking(move || {
        let home = crate::detect::home_dir();
        // 확장 설치는 부가 단계 — 실패해도 폴더 열기는 진행한다
        if let Some(cli) = editor.cli_path(&home) {
            let _ = ensure_extension(&cli, ext);
        }
        open_folder(editor, &path).map_err(AppError::classify)
    })
    .await
    .map_err(|e| AppError::generic(e.to_string()))?
}

/// 편집기 CLI로 확장이 없으면 설치한다 (best-effort).
fn ensure_extension(cli: &Path, ext: &str) -> Result<(), String> {
    let listed = crate::detect::command(cli)
        .arg("--list-extensions")
        .output()
        .map_err(|e| format!("cannot list editor extensions: {e}"))?;
    let installed = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .any(|l| l.trim().eq_ignore_ascii_case(ext));
    if installed {
        return Ok(());
    }
    let status = crate::detect::command(cli)
        .args(["--install-extension", ext, "--force"])
        .status()
        .map_err(|e| format!("cannot install editor extension '{ext}': {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "editor extension installer exited unsuccessfully for '{ext}'"
        ))
    }
}

#[cfg(target_os = "macos")]
fn open_folder(editor: Editor, path: &str) -> Result<(), String> {
    let status = std::process::Command::new("/usr/bin/open")
        .args(["-a", editor.mac_app_name()])
        .arg(path)
        .status()
        .map_err(|e| format!("cannot open editor application: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("editor application exited unsuccessfully while opening the project".into())
    }
}

#[cfg(windows)]
fn open_folder(editor: Editor, path: &str) -> Result<(), String> {
    let home = crate::detect::home_dir();
    let exe = editor
        .installed_path(&home)
        .ok_or("editor executable was not found")?;
    crate::detect::command(&exe)
        .arg(path)
        .spawn()
        .map_err(|e| format!("cannot start editor executable: {e}"))?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn open_folder(_editor: Editor, _path: &str) -> Result<(), String> {
    Err("opening an editor is not supported on this operating system".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_ids_roundtrip() {
        for e in Editor::ALL {
            assert!(Editor::from_id(e.id()).is_some());
            assert!(!e.name().is_empty());
            assert!(e.url().starts_with("https://"));
        }
        assert!(Editor::from_id("emacs").is_none());
    }

    /// 실기기에 설치된 편집기를 감지하는지 확인. 실행: cargo test -- --ignored --nocapture
    #[test]
    #[ignore = "실기기 편집기 설치 상황에 의존"]
    fn detect_editors_on_this_machine() {
        let home = crate::detect::home_dir();
        for e in Editor::ALL {
            println!("{}: {:?}", e.name(), e.installed_path(&home));
        }
    }
}
