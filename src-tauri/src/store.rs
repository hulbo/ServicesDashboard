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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_workspace_name")]
    pub current_workspace: String,
    /// 창을 닫으면 종료하지 않고 트레이로 내린다
    #[serde(default = "yes")]
    pub close_to_tray: bool,
    /// 서비스 준비 완료·비정상 종료 알림
    #[serde(default = "yes")]
    pub notifications: bool,
    #[serde(default)]
    pub log_files: LogFileSettings,
    /// 서비스를 직접 시작·재시작할 때 이전 실행의 콘솔 로그를 지운다 (자동 재시작은 지우지 않음)
    #[serde(default = "yes")]
    pub clear_console_on_start: bool,
    #[serde(default)]
    pub fonts: FontSettings,
}

/// 화면·콘솔 글꼴. 이름이 비어 있으면 기본 글꼴을 쓴다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FontSettings {
    #[serde(default)]
    pub ui: String,
    #[serde(default)]
    pub mono: String,
    /// 콘솔 글자 크기(px)
    #[serde(default = "default_console_size")]
    pub console_size: f32,
}

impl Default for FontSettings {
    fn default() -> Self {
        FontSettings { ui: String::new(), mono: String::new(), console_size: default_console_size() }
    }
}

impl FontSettings {
    pub const MIN_SIZE: f32 = 9.0;
    pub const MAX_SIZE: f32 = 24.0;

    /// 잘못된 값(직접 고친 파일 등)을 쓸 수 있는 값으로 바로잡는다.
    pub fn sanitized(mut self) -> Self {
        self.ui = self.ui.trim().to_string();
        self.mono = self.mono.trim().to_string();
        self.console_size = if self.console_size.is_finite() {
            ((self.console_size * 2.0).round() / 2.0).clamp(Self::MIN_SIZE, Self::MAX_SIZE)
        } else {
            default_console_size()
        };
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogFileSettings {
    #[serde(default)]
    pub enabled: bool,
    /// 이 기간(일)이 지난 로그 파일은 지운다. 0이면 지우지 않는다.
    #[serde(default = "default_retention")]
    pub retention_days: u32,
}

fn default_workspace_name() -> String {
    "default".into()
}

fn yes() -> bool {
    true
}

fn default_retention() -> u32 {
    7
}

fn default_console_size() -> f32 {
    12.5
}

impl Default for LogFileSettings {
    fn default() -> Self {
        LogFileSettings { enabled: false, retention_days: default_retention() }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            current_workspace: default_workspace_name(),
            close_to_tray: true,
            notifications: true,
            log_files: LogFileSettings::default(),
            clear_console_on_start: true,
            fonts: FontSettings::default(),
        }
    }
}

pub struct Store {
    pub base_dir: PathBuf,
    pub workspace_path: PathBuf,
    pub gradle_init_script: PathBuf,
    pub gradle_classpath_script: PathBuf,
    pub settings_path: PathBuf,
    pub settings: Settings,
}

impl Store {
    pub fn default_base_dir() -> PathBuf {
        match custom_base_dir() {
            Some(home) => PathBuf::from(home),
            None => dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("ServicesDashboard"),
        }
    }

    /// SERVICES_DASHBOARD_HOME으로 설정 폴더를 따로 지정했는지
    pub fn uses_custom_base_dir() -> bool {
        custom_base_dir().is_some()
    }

    pub fn open(base_dir: PathBuf) -> Result<Self, String> {
        let ws_dir = base_dir.join("workspaces");
        let gradle_dir = base_dir.join("gradle");
        for d in [&base_dir, &ws_dir, &gradle_dir] {
            fs::create_dir_all(d).map_err(|e| format!("설정 폴더를 만들 수 없습니다: {} ({e})", d.display()))?;
        }

        let settings_path = base_dir.join("settings.json");
        let mut settings: Settings = match fs::read_to_string(&settings_path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => {
                let s = Settings::default();
                write_atomic(&settings_path, &serde_json::to_string_pretty(&s).unwrap())?;
                s
            }
        };

        settings.fonts = settings.fonts.sanitized();

        let file_name = sanitize_file_name(&settings.current_workspace);
        let workspace_path = ws_dir.join(format!("{file_name}.json"));

        // init script는 항상 최신 내용으로 덮어쓴다.
        let gradle_init_script = gradle_dir.join("sd-init.gradle");
        write_atomic(&gradle_init_script, GRADLE_INIT_SCRIPT)?;

        let gradle_classpath_script = gradle_dir.join("sd-classpath.gradle");
        write_atomic(&gradle_classpath_script, crate::javamain::GRADLE_CLASSPATH_SCRIPT)?;

        Ok(Store { base_dir, workspace_path, gradle_init_script, gradle_classpath_script, settings_path, settings })
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    /// 앱 설정을 저장한다 (현재 워크스페이스 이름은 바꾸지 않는다).
    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        let mut s = settings.clone();
        s.current_workspace = self.settings.current_workspace.clone();
        write_atomic(&self.settings_path, &serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?)
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

fn custom_base_dir() -> Option<String> {
    std::env::var("SERVICES_DASHBOARD_HOME").ok().filter(|h| !h.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Service;

    #[test]
    fn font_settings_default_and_sanitize() {
        // 예전 설정 파일(글꼴 항목 없음)은 기본값
        let old: Settings = serde_json::from_str(r#"{"currentWorkspace":"default"}"#).unwrap();
        assert_eq!(old.fonts, FontSettings::default());
        assert_eq!(old.fonts.console_size, 12.5);
        let f = FontSettings { ui: "  D2Coding ".into(), mono: String::new(), console_size: 40.0 }.sanitized();
        assert_eq!((f.ui.as_str(), f.console_size), ("D2Coding", FontSettings::MAX_SIZE));
        assert_eq!(FontSettings { console_size: 13.3, ..Default::default() }.sanitized().console_size, 13.5);
        assert_eq!(FontSettings { console_size: 1.0, ..Default::default() }.sanitized().console_size, FontSettings::MIN_SIZE);
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["fonts"], serde_json::json!({ "ui": "", "mono": "", "consoleSize": 12.5 }));
    }

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
    fn settings_defaults_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), r#"{"currentWorkspace":"w1"}"#).unwrap();
        let store = Store::open(dir.path().to_path_buf()).unwrap();
        assert!(store.workspace_path.ends_with("w1.json"));
        assert!(store.settings.close_to_tray && store.settings.notifications);
        assert!(!store.settings.log_files.enabled);
        let mut s = store.settings.clone();
        s.close_to_tray = false;
        s.log_files.enabled = true;
        s.current_workspace = "다른 이름".into();
        store.save_settings(&s).unwrap();
        let again = Store::open(dir.path().to_path_buf()).unwrap();
        assert!(!again.settings.close_to_tray && again.settings.log_files.enabled);
        assert_eq!(again.settings.current_workspace, "w1", "워크스페이스 이름은 설정 저장으로 바뀌지 않는다");
    }

    #[test]
    fn file_name_is_sanitized() {
        assert_eq!(sanitize_file_name("my ws/1"), "my_ws_1");
        assert_eq!(sanitize_file_name(".."), "default");
        assert_eq!(sanitize_file_name("회사"), "회사");
    }
}
