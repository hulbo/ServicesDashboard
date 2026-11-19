//! 프론트엔드에서 호출하는 IPC 명령.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::launch::{self, LaunchContext, LaunchPreview, LaunchSpec};
use crate::logs::{LogBatch, LogHub, LogLine, STREAM_SYS};
use crate::model::{Service, Workspace};
use crate::process::{EventSink, ProcessManager, ServiceStatus};
use crate::store::Store;

pub const EVENT_STATE: &str = "service://state";
pub const EVENT_LOGS: &str = "service://logs";

pub struct TauriSink(pub AppHandle);

impl EventSink for TauriSink {
    fn status(&self, status: &ServiceStatus) {
        let _ = self.0.emit(EVENT_STATE, status);
    }
    fn logs(&self, batches: Vec<LogBatch>) {
        let _ = self.0.emit(EVENT_LOGS, batches);
    }
}

pub struct AppCore {
    pub store: Store,
    /// 파일을 읽지 못했으면 오류 메시지를 들고 있는다 (그 상태에서는 저장을 막는다).
    pub workspace: Mutex<Result<Workspace, String>>,
    pub manager: Arc<ProcessManager>,
    pub logs: Arc<LogHub>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub workspace: Workspace,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub config_dir: String,
    pub workspace_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartError {
    pub id: String,
    pub name: String,
    pub error: String,
}

impl AppCore {
    fn current(&self) -> Result<Workspace, String> {
        self.workspace.lock().unwrap().clone()
    }

    fn info(&self) -> Result<WorkspaceInfo, String> {
        Ok(WorkspaceInfo { workspace: self.current()?, path: self.store.workspace_path.to_string_lossy().into_owned() })
    }

    fn build_spec(&self, ws: &Workspace, svc: &Service) -> Result<LaunchSpec, String> {
        let dir = self.store.workspace_dir();
        launch::build(
            &LaunchContext { workspace: ws, workspace_dir: &dir, gradle_init_script: &self.store.gradle_init_script },
            svc,
        )
    }

    pub fn start(&self, id: &str) -> Result<ServiceStatus, String> {
        let ws = self.current()?;
        let svc = ws.service(id).ok_or_else(|| format!("서비스를 찾을 수 없습니다: {id}"))?;
        match self.build_spec(&ws, svc) {
            Ok(spec) => self.manager.start(id, spec),
            Err(e) => {
                self.logs.push(id, STREAM_SYS, format!("✕ 실행할 수 없습니다: {e}"));
                Err(e)
            }
        }
    }

    fn group_service_ids(&self, group_id: &str) -> Result<Vec<(String, String)>, String> {
        let ws = self.current()?;
        Ok(ws
            .services
            .iter()
            .filter(|s| s.group.as_deref() == Some(group_id))
            .map(|s| (s.id.clone(), s.name.clone()))
            .collect())
    }
}

type Core<'a> = State<'a, Arc<AppCore>>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn app_info(core: Core) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        config_dir: core.store.base_dir.to_string_lossy().into_owned(),
        workspace_path: core.store.workspace_path.to_string_lossy().into_owned(),
    }
}

#[tauri::command]
pub fn workspace_get(core: Core) -> Result<WorkspaceInfo, String> {
    core.info()
}

#[tauri::command]
pub fn workspace_reload(core: Core) -> Result<WorkspaceInfo, String> {
    let loaded = core.store.load();
    *core.workspace.lock().unwrap() = loaded;
    core.info()
}

#[tauri::command]
pub fn workspace_save(core: Core, workspace: Workspace) -> Result<Workspace, String> {
    if let Err(e) = core.current() {
        return Err(format!("워크스페이스 파일 오류를 먼저 해결하세요: {e}"));
    }
    core.store.save(&workspace)?;
    *core.workspace.lock().unwrap() = Ok(workspace.clone());

    // 삭제된 서비스가 실행 중이면 중지한다.
    let orphans: Vec<String> =
        core.manager.running_ids().into_iter().filter(|id| workspace.service(id).is_none()).collect();
    if !orphans.is_empty() {
        let manager = core.manager.clone();
        std::thread::spawn(move || {
            let _ = manager.stop_many(&orphans);
        });
    }
    Ok(workspace)
}

#[tauri::command]
pub fn service_statuses(core: Core) -> Vec<ServiceStatus> {
    core.manager.statuses()
}

#[tauri::command]
pub async fn service_start(core: Core<'_>, id: String) -> Result<ServiceStatus, String> {
    let core = core.inner().clone();
    blocking(move || core.start(&id)).await
}

#[tauri::command]
pub async fn service_stop(core: Core<'_>, id: String) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || core.manager.stop(&id)).await
}

#[tauri::command]
pub async fn service_restart(core: Core<'_>, id: String) -> Result<ServiceStatus, String> {
    let core = core.inner().clone();
    blocking(move || {
        core.manager.stop(&id)?;
        core.start(&id)
    })
    .await
}

#[tauri::command]
pub async fn group_start(core: Core<'_>, id: String) -> Result<Vec<StartError>, String> {
    let core = core.inner().clone();
    blocking(move || {
        let mut errors = Vec::new();
        for (sid, name) in core.group_service_ids(&id)? {
            if core.manager.is_running(&sid) {
                continue;
            }
            if let Err(error) = core.start(&sid) {
                errors.push(StartError { id: sid, name, error });
            }
        }
        Ok(errors)
    })
    .await
}

#[tauri::command]
pub async fn group_stop(core: Core<'_>, id: String) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || {
        let ids: Vec<String> = core.group_service_ids(&id)?.into_iter().map(|(sid, _)| sid).collect();
        core.manager.stop_many(&ids)
    })
    .await
}

#[tauri::command]
pub async fn stop_all(core: Core<'_>) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || {
        let ids = core.manager.running_ids();
        core.manager.stop_many(&ids)
    })
    .await
}

#[tauri::command]
pub fn logs_get(core: Core, id: String) -> Vec<LogLine> {
    core.logs.snapshot(&id)
}

#[tauri::command]
pub fn logs_clear(core: Core, id: String) {
    core.logs.clear(&id);
}

/// 저장 전 편집 중인 서비스로 실제 실행 명령을 미리 계산한다.
#[tauri::command]
pub fn launch_preview(core: Core, service: Service) -> Result<LaunchPreview, String> {
    let ws = core.current()?;
    core.build_spec(&ws, &service).map(|s| LaunchPreview::from(&s))
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("http(s) 주소만 열 수 있습니다.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}
