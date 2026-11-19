//! 프론트엔드에서 호출하는 IPC 명령.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::importers::{self, ScanContext, ScanResult};
use crate::launch::{self, LaunchContext, LaunchPreview, LaunchSpec};
use crate::logfile::{self, LogFiles};
use crate::logs::{LogBatch, LogHub, LogLine, STREAM_SYS};
use crate::model::{Service, Workspace};
use crate::ports::{self, PortOwner};
use crate::process::{EventSink, ProcessManager, ServiceStatus};
use crate::runtime::{self, DetectEnv, RuntimeInfo};
use crate::store::{Settings, Store};
use crate::vars::VarContext;
use crate::watch::{self, ReadySpec};

pub const EVENT_STATE: &str = "service://state";
pub const EVENT_LOGS: &str = "service://logs";

pub struct TauriSink(pub AppHandle);

impl EventSink for TauriSink {
    fn status(&self, status: &ServiceStatus) {
        let _ = self.0.emit(EVENT_STATE, status);
        crate::shell::on_status(&self.0, status);
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
    /// 감지된 런타임 (처음 필요할 때 또는 앱 시작 직후 백그라운드에서 채운다)
    pub runtimes: Mutex<Option<Vec<RuntimeInfo>>>,
    pub detect_env: DetectEnv,
    pub settings: Mutex<Settings>,
    pub files: Arc<LogFiles>,
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

    /// 런타임 목록. 아직 감지하지 않았거나 refresh면 지금 감지한다.
    pub fn runtimes(&self, refresh: bool) -> Vec<RuntimeInfo> {
        let mut guard = self.runtimes.lock().unwrap();
        if refresh || guard.is_none() {
            *guard = Some(runtime::detect(&self.detect_env));
        }
        guard.clone().unwrap_or_default()
    }

    fn build_spec(&self, ws: &Workspace, svc: &Service) -> Result<LaunchSpec, String> {
        let dir = self.store.workspace_dir();
        let runtimes = self.runtimes(false);
        launch::build(
            &LaunchContext {
                workspace: ws,
                workspace_dir: &dir,
                gradle_init_script: &self.store.gradle_init_script,
                runtimes: &runtimes,
            },
            svc,
        )
    }

    pub fn scan(&self, root: &str) -> Result<ScanResult, String> {
        let runtimes = self.runtimes(false);
        importers::scan(
            std::path::Path::new(root.trim()),
            &ScanContext {
                runtimes: &runtimes,
                jetbrains_dirs: &self.detect_env.jetbrains_dirs,
                user_home: self.detect_env.user_home.clone(),
                gradle_init_script: &self.store.gradle_init_script,
            },
        )
    }

    fn ready_spec(&self, ws: &Workspace, svc: &Service) -> Result<Option<ReadySpec>, String> {
        let dir = self.store.workspace_dir();
        let vars = VarContext::new(&ws.variables, &dir.to_string_lossy());
        let http = vars.expand_opt(svc.readiness.as_ref().and_then(|r| r.http_url.as_ref()))?;
        watch::ready_spec(svc, http)
    }

    /// 서비스가 쓸 포트(설정한 포트 + 지난번 실행에서 감지한 포트)를 점유한 프로세스
    pub fn port_conflicts(&self, id: &str) -> Result<Vec<PortOwner>, String> {
        let ws = self.current()?;
        let svc = ws.service(id).ok_or_else(|| format!("서비스를 찾을 수 없습니다: {id}"))?;
        let mut wanted = svc.ports.clone();
        for p in self.manager.known_ports(id) {
            if !wanted.contains(&p) {
                wanted.push(p);
            }
        }
        if wanted.is_empty() || self.manager.is_running(id) {
            return Ok(Vec::new());
        }
        let mut owners = ports::owners(&wanted);
        for o in &mut owners {
            o.service_id = self.manager.service_owning(o.pid);
        }
        Ok(owners)
    }

    pub fn start(&self, id: &str) -> Result<ServiceStatus, String> {
        let ws = self.current()?;
        let svc = ws.service(id).ok_or_else(|| format!("서비스를 찾을 수 없습니다: {id}"))?;
        let prepared = self.build_spec(&ws, svc).and_then(|spec| Ok((spec, self.ready_spec(&ws, svc)?)));
        let (spec, ready) = match prepared {
            Ok(x) => x,
            Err(e) => {
                self.logs.push(id, STREAM_SYS, format!("✕ 실행할 수 없습니다: {e}"));
                return Err(e);
            }
        };
        let conflicts = self.port_conflicts(id)?;
        if !conflicts.is_empty() {
            let names = ws.services.iter().map(|s| (s.id.as_str(), s.name.as_str())).collect::<std::collections::HashMap<_, _>>();
            let list = conflicts
                .iter()
                .map(|o| match o.service_id.as_deref().and_then(|sid| names.get(sid)) {
                    Some(n) => format!("{} (서비스 '{n}')", o.port),
                    None => format!("{} ({}, PID {})", o.port, o.name, o.pid),
                })
                .collect::<Vec<_>>()
                .join(", ");
            let msg = format!("포트가 이미 사용 중입니다: {list}");
            self.logs.push(id, STREAM_SYS, format!("✕ {msg}"));
            return Err(msg);
        }
        self.manager.start(id, spec, ready)
    }

    pub fn start_group(&self, group_id: &str) -> Vec<StartError> {
        let mut errors = Vec::new();
        let ids = match self.group_service_ids(group_id) {
            Ok(ids) => ids,
            Err(error) => return vec![StartError { id: group_id.into(), name: group_id.into(), error }],
        };
        for (sid, name) in ids {
            if self.manager.is_running(&sid) {
                continue;
            }
            if let Err(error) = self.start(&sid) {
                errors.push(StartError { id: sid, name, error });
            }
        }
        errors
    }

    pub fn stop_group(&self, group_id: &str) -> Result<(), String> {
        let ids: Vec<String> = self.group_service_ids(group_id)?.into_iter().map(|(sid, _)| sid).collect();
        self.manager.stop_many(&ids)
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
pub fn workspace_reload(app: AppHandle, core: Core) -> Result<WorkspaceInfo, String> {
    let loaded = core.store.load();
    *core.workspace.lock().unwrap() = loaded;
    crate::shell::refresh_tray(&app);
    core.info()
}

#[tauri::command]
pub fn workspace_save(app: AppHandle, core: Core, workspace: Workspace) -> Result<Workspace, String> {
    if let Err(e) = core.current() {
        return Err(format!("워크스페이스 파일 오류를 먼저 해결하세요: {e}"));
    }
    core.store.save(&workspace)?;
    *core.workspace.lock().unwrap() = Ok(workspace.clone());
    crate::shell::refresh_tray(&app);

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
    blocking(move || Ok(core.start_group(&id))).await
}

#[tauri::command]
pub async fn group_stop(core: Core<'_>, id: String) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || core.stop_group(&id)).await
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
pub async fn ports_check(core: Core<'_>, id: String) -> Result<Vec<PortOwner>, String> {
    let core = core.inner().clone();
    blocking(move || core.port_conflicts(&id)).await
}

/// 포트를 점유한 프로세스를 끝낸다. 대시보드 서비스면 그 서비스를 중지한다.
#[tauri::command]
pub async fn process_kill(core: Core<'_>, pid: u32) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || match core.manager.service_owning(pid) {
        Some(sid) => core.manager.stop(&sid),
        None => ports::kill_tree(pid),
    })
    .await
}

#[tauri::command]
pub fn settings_get(core: Core) -> Settings {
    core.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn settings_save(core: Core, settings: Settings) -> Result<Settings, String> {
    core.store.save_settings(&settings)?;
    core.files.configure(settings.log_files.enabled, settings.log_files.retention_days);
    core.files.cleanup();
    *core.settings.lock().unwrap() = settings.clone();
    Ok(settings)
}

#[tauri::command]
pub fn logs_export(core: Core, id: String, path: String) -> Result<usize, String> {
    logfile::export(&core.logs.snapshot(&id), std::path::Path::new(&path))
}

/// 서비스 로그 파일 폴더 (없으면 만든다)
#[tauri::command]
pub fn logs_dir(core: Core, id: String) -> Result<String, String> {
    let dir = core.files.service_dir(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn runtimes_detect(core: Core<'_>, refresh: bool) -> Result<Vec<RuntimeInfo>, String> {
    let core = core.inner().clone();
    blocking(move || Ok(core.runtimes(refresh))).await
}

#[tauri::command]
pub async fn project_scan(core: Core<'_>, root: String) -> Result<ScanResult, String> {
    let core = core.inner().clone();
    blocking(move || core.scan(&root)).await
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
