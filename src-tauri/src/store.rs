//! 설정 파일 저장소.
//!
//! 기본 위치: `%APPDATA%\ServicesDashboard\`
//! - `settings.json`             앱 설정 (현재 워크스페이스 이름 등)
//! - `workspaces\<이름>.json`     서비스 정의
//! - `gradle\sd-init.gradle`      Gradle JVM 옵션 주입 스크립트
//!
//! 환경변수 `SERVICES_DASHBOARD_HOME`으로 위치를 바꿀 수 있다 (포터블 사용, 테스트).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::launch::GRADLE_INIT_SCRIPT;
use crate::model::Workspace;

pub const SCHEMA_URL: &str = "https://raw.githubusercontent.com/hulbo/ServicesDashboard/main/schema/workspace.schema.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_workspace_name")]
    pub current_workspace: String,
}

fn default_workspace_name() -> String {
    "default".into()
}

impl Default for Settings {
    fn default() -> Self {
        Settings { current_workspace: default_workspace_name() }
    }
}

pub struct Store {
    pub base_dir: PathBuf,
    pub workspace_path: PathBuf,
    pub gradle_init_script: PathBuf,
}

impl Store {
    pub fn default_base_dir() -> PathBuf {
        if let Ok(home) = std::env::var("SERVICES_DASHBOARD_HOME") {
            if !home.trim().is_empty() {
                return PathBuf::from(home);
            }
        }
        dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("ServicesDashboard")
    }

    pub fn open(base_dir: PathBuf) -> Result<Self, String> {
        let ws_dir = base_dir.join("workspaces");
        let gradle_dir = base_dir.join("gradle");
        for d in [&base_dir, &ws_dir, &gradle_dir] {
            fs::create_dir_all(d).map_err(|e| format!("설정 폴더를 만들 수 없습니다: {} ({e})", d.display()))?;
        }

        let settings_path = base_dir.join("settings.json");
        let settings: Settings = match fs::read_to_string(&settings_path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => {
                let s = Settings::default();
                write_atomic(&settings_path, &serde_json::to_string_pretty(&s).unwrap())?;
                s
            }
        };

        let file_name = sanitize_file_name(&settings.current_workspace);
        let workspace_path = ws_dir.join(format!("{file_name}.json"));

        // init script는 항상 최신 내용으로 덮어쓴다.
        let gradle_init_script = gradle_dir.join("sd-init.gradle");
        write_atomic(&gradle_init_script, GRADLE_INIT_SCRIPT)?;

        Ok(Store { base_dir, workspace_path, gradle_init_script })
    }

    pub fn workspace_dir(&self) -> PathBuf {
        self.workspace_path.parent().map(Path::to_path_buf).unwrap_or_else(|| self.base_dir.clone())
    }

    /// 파일이 없으면 빈 워크스페이스를 만들어 저장한다. 파일이 깨졌으면 덮어쓰지 않고 오류를 돌려준다.
    pub fn load(&self) -> Result<Workspace, String> {
        match fs::read_to_string(&self.workspace_path) {
            Ok(text) => serde_json::from_str::<Workspace>(&text).map_err(|e| {
                format!("워크스페이스 파일을 해석할 수 없습니다: {} ({e})", self.workspace_path.display())
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let name = self.workspace_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let mut ws = Workspace::empty(&name);
                ws.schema = Some(SCHEMA_URL.into());
                self.save(&ws)?;
                Ok(ws)
            }
            Err(e) => Err(format!("워크스페이스 파일을 읽을 수 없습니다: {} ({e})", self.workspace_path.display())),
        }
    }

    pub fn save(&self, ws: &Workspace) -> Result<(), String> {
        ws.validate()?;
        let text = serde_json::to_string_pretty(ws).map_err(|e| e.to_string())?;
        write_atomic(&self.workspace_path, &(text + "\n"))
    }
}

fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    if cleaned.trim_matches('.').is_empty() { "default".into() } else { cleaned }
}

/// 임시 파일에 쓴 뒤 이름을 바꿔, 쓰는 도중 앱이 꺼져도 기존 파일이 깨지지 않게 한다.
fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content).map_err(|e| format!("파일을 쓸 수 없습니다: {} ({e})", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("파일을 저장할 수 없습니다: {} ({e})", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Service;

    #[test]
    fn creates_layout_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().to_path_buf()).unwrap();
        assert!(store.gradle_init_script.is_file());
        let mut ws = store.load().unwrap();
        assert_eq!(ws.name, "default");
        assert!(store.workspace_path.is_file());
        ws.services.push(Service { id: "a".into(), name: "A".into(), ..Default::default() });
        store.save(&ws).unwrap();
        assert_eq!(store.load().unwrap(), ws);
    }

    #[test]
    fn broken_file_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().to_path_buf()).unwrap();
        fs::write(&store.workspace_path, "{ broken").unwrap();
        assert!(store.load().unwrap_err().contains("해석할 수 없습니다"));
        assert_eq!(fs::read_to_string(&store.workspace_path).unwrap(), "{ broken");
    }

    #[test]
    fn file_name_is_sanitized() {
        assert_eq!(sanitize_file_name("my ws/1"), "my_ws_1");
        assert_eq!(sanitize_file_name(".."), "default");
        assert_eq!(sanitize_file_name("회사"), "회사");
    }
}
