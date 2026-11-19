//! 프론트엔드에서 호출하는 IPC 명령.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::git;
use crate::importers::{self, ScanContext, ScanResult};
use crate::javamain;
use crate::launch::{self, JavaClasspath, LaunchContext, LaunchPreview, LaunchSpec};
use crate::logfile::{self, LogFiles};
use crate::logs::{LogBatch, LogHub, LogLine, STREAM_SYS};
use crate::model::{ClasspathSource, Service, ServiceKind, Workspace};
use crate::ports::{self, PortOwner};
use crate::process::{CANCELLED, EventSink, ProcessManager, RunState, ServiceStatus};
use crate::runtime::{self, DetectEnv, RuntimeInfo};
use crate::store::{Settings, Store};
use crate::vars::VarContext;
use crate::watch::{self, ReadySpec};

pub const EVENT_STATE: &str = "service://state";
pub const EVENT_LOGS: &str = "service://logs";
pub const EVENT_START_ERROR: &str = "service://start-error";

/// 의존 서비스 대기가 이 시간을 넘으면 포기한다.
const LAUNCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

pub struct TauriSink(pub AppHandle);

impl EventSink for TauriSink {
    fn status(&self, status: &ServiceStatus) {
        let _ = self.0.emit(EVENT_STATE, status);
        crate::shell::on_status(&self.0, status);
        if let Some(core) = tauri::Manager::try_state::<Arc<AppCore>>(&self.0) {
            core.inner().clone().on_status(status);
        }
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
    /// 서비스별 연속 자동 재시작 횟수
    pub restarts: Mutex<std::collections::HashMap<String, u32>>,
    /// 백그라운드 시작 실패를 화면에 알린다
    pub on_error: Box<dyn Fn(&StartError) + Send + Sync>,
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

#[derive(Serialize, Clone, Debug)]
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

    fn build_spec(&self, ws: &Workspace, svc: &Service, debug: bool, cp: Option<&JavaClasspath>) -> Result<LaunchSpec, String> {
        let dir = self.store.workspace_dir();
        let runtimes = self.runtimes(false);
        launch::build(
            &LaunchContext {
                workspace: ws,
                workspace_dir: &dir,
                gradle_init_script: &self.store.gradle_init_script,
                runtimes: &runtimes,
                debug,
                java_classpath: cp,
            },
            svc,
        )
    }

    fn classpath_files(&self, id: &str) -> javamain::CacheFiles {
        javamain::cache_files(&self.store.base_dir.join("classpath"), id)
    }

    fn expanded_cwd(&self, ws: &Workspace, svc: &Service) -> Result<std::path::PathBuf, String> {
        let dir = self.store.workspace_dir();
        let vars = VarContext::new(&ws.variables, &dir.to_string_lossy());
        let cwd = vars.expand_opt(svc.cwd.as_ref())?.ok_or("작업 디렉터리를 지정하세요.")?;
        let p = std::path::PathBuf::from(cwd);
        Ok(if p.is_absolute() { p } else { dir.join(p) })
    }

    /// java-main: 필요하면 빌드 도구로 컴파일·클래스패스를 계산하고, 캐시된 클래스패스를 돌려준다.
    fn prepare_java_main(&self, ws: &Workspace, svc: &Service) -> Result<JavaClasspath, String> {
        let jm = svc.java_main.clone().unwrap_or_default();
        let files = self.classpath_files(&svc.id);
        let cwd = self.expanded_cwd(ws, svc)?;
        let entries = match jm.source {
            ClasspathSource::Manual => {
                let dir = self.store.workspace_dir();
                let vars = VarContext::new(&ws.variables, &dir.to_string_lossy());
                let mut out = Vec::new();
                for e in &jm.classpath {
                    let p = std::path::PathBuf::from(vars.expand(e)?);
                    out.push(if p.is_absolute() { p } else { cwd.join(p) }.to_string_lossy().replace('\\', "/"));
                }
                if out.is_empty() {
                    return Err("클래스패스를 입력하세요 (직접 지정 방식).".into());
                }
                out
            }
            ClasspathSource::Gradle | ClasspathSource::Maven => {
                if javamain::needs_build(jm.rebuild, &files.classpath, &cwd) {
                    let prep = javamain::prepare_service(svc, &files, &self.store.gradle_classpath_script)
                        .ok_or("준비 단계를 만들 수 없습니다.")?;
                    let spec = self.build_spec(ws, &prep, false, None)?;
                    let _ = std::fs::remove_file(&files.classpath);
                    self.manager.run_prepare(&svc.id, spec)?;
                    if jm.source == ClasspathSource::Maven {
                        let module_dir = match jm.module.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
                            Some(m) => cwd.join(m),
                            None => cwd.clone(),
                        };
                        javamain::finish_maven(&files, &module_dir)?;
                    } else if !files.classpath.is_file() {
                        return Err("Gradle이 클래스패스를 만들지 못했습니다. java 플러그인이 적용된 프로젝트 경로인지 확인하세요.".into());
                    }
                } else {
                    self.logs.push(&svc.id, STREAM_SYS, "⚡ 바뀐 소스·빌드 파일이 없어 빌드를 건너뜁니다 (캐시된 클래스패스 사용)".to_string());
                }
                javamain::read_classpath(&files.classpath)?
            }
        };
        javamain::write_argfile(&files.argfile, &entries)?;
        Ok(JavaClasspath { entries, argfile: files.argfile })
    }

    /// 미리보기용: 캐시가 있으면 그것을, 없으면 자리표시를 쓴다.
    fn preview_classpath(&self, svc: &Service) -> JavaClasspath {
        let files = self.classpath_files(&svc.id);
        let entries = javamain::read_classpath(&files.classpath).unwrap_or_else(|_| vec!["<시작할 때 계산>".into()]);
        JavaClasspath { entries, argfile: files.argfile }
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

    /// 서비스가 쓸 포트(설정한 포트 + 지난번 실행에서 감지한 포트 + 디버그 포트)를 점유한 프로세스
    pub fn port_conflicts(&self, id: &str) -> Result<Vec<PortOwner>, String> {
        self.port_conflicts_with(id, &[])
    }

    fn port_conflicts_with(&self, id: &str, extra: &[u16]) -> Result<Vec<PortOwner>, String> {
        let ws = self.current()?;
        let svc = ws.service(id).ok_or_else(|| format!("서비스를 찾을 수 없습니다: {id}"))?;
        let mut wanted = svc.ports.clone();
        wanted.extend(extra.iter().filter(|p| !svc.ports.contains(p)));
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

    /// 서비스 하나를 바로 시작한다 (의존 관계는 보지 않는다. 보통은 `launch`를 쓴다).
    pub fn start(&self, id: &str, debug: bool) -> Result<ServiceStatus, String> {
        let ws = self.current()?;
        let svc = ws.service(id).ok_or_else(|| format!("서비스를 찾을 수 없습니다: {id}"))?;
        let debug_port = debug.then(|| {
            let java = !matches!(svc.kind, ServiceKind::Npm | ServiceKind::Node | ServiceKind::Shell);
            svc.debug.as_ref().and_then(|d| d.port).unwrap_or(if java { 5005 } else { 9229 })
        });
        let conflicts = self.port_conflicts_with(id, &debug_port.into_iter().collect::<Vec<_>>())?;
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
        let cp = if svc.kind == ServiceKind::JavaMain {
            match self.prepare_java_main(&ws, svc) {
                Ok(cp) => Some(cp),
                Err(e) => {
                    if e != CANCELLED && !e.starts_with("준비(빌드)") {
                        self.logs.push(id, STREAM_SYS, format!("✕ 실행할 수 없습니다: {e}"));
                    }
                    return Err(e);
                }
            }
        } else {
            None
        };
        let prepared = self.build_spec(&ws, svc, debug, cp.as_ref()).and_then(|spec| Ok((spec, self.ready_spec(&ws, svc)?)));
        let (spec, ready) = match prepared {
            Ok(x) => x,
            Err(e) => {
                self.logs.push(id, STREAM_SYS, format!("✕ 실행할 수 없습니다: {e}"));
                return Err(e);
            }
        };
        self.manager.start(id, spec, ready)
    }

    fn is_active(&self, id: &str) -> bool {
        self.manager.is_running(id)
            || self.manager.is_preparing(id)
            || matches!(self.manager.status(id).state, RunState::Starting | RunState::Running | RunState::Stopping)
    }

    fn report(&self, id: &str, name: &str, error: &str) {
        (self.on_error)(&StartError { id: id.into(), name: name.into(), error: error.into() });
    }

    /// 서비스들을 의존 순서대로 시작한다. 의존 대상이 준비될 때까지 '대기 중'으로 두고,
    /// 실제 시작은 백그라운드에서 한다 (실패는 on_error로 알린다).
    /// `manual`이면 사용자가 직접 시작한 것으로 보고 자동 재시작 횟수를 초기화하고,
    /// (설정이 켜져 있으면) 새로 시작하는 서비스의 이전 콘솔 로그를 지운다.
    pub fn launch(self: &Arc<Self>, ids: Vec<String>, debug: bool, manual: bool) -> Result<(), String> {
        self.launch_opts(ids, debug, manual, manual)
    }

    /// 새로 시작할 서비스 (의존 대상 포함, 이미 실행 중인 것 제외)
    fn to_start(&self, ws: &Workspace, ids: &[String]) -> Vec<String> {
        ws.launch_order(ids).into_iter().filter(|id| !self.is_active(id)).collect()
    }

    fn clear_on_start(&self) -> bool {
        self.settings.lock().unwrap().clear_console_on_start
    }

    fn launch_opts(self: &Arc<Self>, ids: Vec<String>, debug: bool, manual: bool, reset_logs: bool) -> Result<(), String> {
        let ws = self.current()?;
        for id in &ids {
            ws.service(id).ok_or_else(|| format!("서비스를 찾을 수 없습니다: {id}"))?;
        }
        if manual {
            let mut r = self.restarts.lock().unwrap();
            for id in &ids {
                r.remove(id);
            }
        }
        let explicit: std::collections::HashSet<String> = ids.iter().cloned().collect();
        let pending = self.to_start(&ws, &ids);
        if pending.is_empty() {
            return Ok(());
        }
        let reset_logs = reset_logs && self.clear_on_start();
        for id in &pending {
            if reset_logs {
                self.logs.reset(id);
            }
            self.manager.set_waiting(id);
        }
        let me = Arc::clone(self);
        std::thread::Builder::new()
            .name("launch".into())
            .spawn(move || me.orchestrate(ws, pending, explicit, debug))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn orchestrate(&self, ws: Workspace, mut pending: Vec<String>, explicit: std::collections::HashSet<String>, debug: bool) {
        let began = std::time::Instant::now();
        let mut launched: std::collections::HashSet<String> = std::collections::HashSet::new();
        let name = |id: &str| ws.service(id).map(|s| s.name.clone()).unwrap_or_else(|| id.to_string());
        while !pending.is_empty() {
            let mut next = Vec::new();
            for id in std::mem::take(&mut pending) {
                if !self.manager.is_waiting(&id) {
                    continue; // 사용자가 취소함
                }
                let Some(svc) = ws.service(&id) else { continue };
                let mut ready = true;
                let mut blocked = None;
                for d in &svc.depends_on {
                    match self.manager.status(d).state {
                        RunState::Running => {}
                        RunState::Exited if launched.contains(d) => {}
                        RunState::Waiting | RunState::Starting => ready = false,
                        _ => {
                            blocked = Some(d.clone());
                            break;
                        }
                    }
                }
                if let Some(d) = blocked {
                    let msg = format!("의존 서비스 '{}'이(가) 실행되지 않아 시작하지 않았습니다.", name(&d));
                    self.manager.fail_waiting(&id, &msg);
                    self.report(&id, &name(&id), &msg);
                    continue;
                }
                if !ready {
                    next.push(id);
                    continue;
                }
                launched.insert(id.clone());
                if let Err(e) = self.start(&id, debug && explicit.contains(&id)) {
                    if e != CANCELLED {
                        self.manager.fail_waiting_with(&id, &e, false);
                        self.report(&id, &name(&id), &e);
                    }
                }
            }
            pending = next;
            if pending.is_empty() {
                break;
            }
            if began.elapsed() > LAUNCH_TIMEOUT {
                for id in &pending {
                    self.manager.fail_waiting(id, "의존 서비스를 기다리다 시간이 초과됐습니다.");
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    /// 비정상 종료 시 자동 재시작
    pub fn on_status(self: Arc<Self>, st: &ServiceStatus) {
        // 실행 실패·준비 실패·의존 실패(error가 있는 경우)는 다시 해도 같으므로 재시작하지 않는다
        if st.state != RunState::Failed || st.error.is_some() || st.started_at.is_none() {
            return;
        }
        let Ok(ws) = self.current() else { return };
        let Some(svc) = ws.service(&st.id) else { return };
        let Some(cfg) = svc.auto_restart.clone() else { return };
        let ran = st.ended_at.unwrap_or(0).saturating_sub(st.started_at.unwrap_or(0));
        let (attempt, delay) = {
            let mut r = self.restarts.lock().unwrap();
            let n = r.entry(st.id.clone()).or_insert(0);
            if ran > 60_000 {
                *n = 0;
            }
            if *n >= cfg.max() {
                self.logs.push(&st.id, STREAM_SYS, format!("⟳ 자동 재시작 한도({}회)에 도달해 멈춥니다.", cfg.max()));
                return;
            }
            let delay = cfg.delay(*n);
            *n += 1;
            (*n, delay)
        };
        self.logs.push(&st.id, STREAM_SYS, format!("⟳ {}초 뒤 자동 재시작합니다 ({attempt}/{})", delay.as_secs(), cfg.max()));
        let id = st.id.clone();
        let run_id = st.run_id;
        let me = Arc::clone(&self);
        let _ = std::thread::Builder::new().name(format!("restart-{id}")).spawn(move || {
            std::thread::sleep(delay);
            let cur = me.manager.status(&id);
            // 그 사이 사용자가 다시 시작·중지했으면 하지 않는다
            if cur.state == RunState::Failed && cur.run_id == run_id {
                let _ = me.launch(vec![id], false, false);
            }
        });
    }

    pub fn stop_ids(&self, ids: &[String]) -> Result<(), String> {
        self.manager.stop_many(ids)
    }

    pub fn group_ids(&self, group_id: &str) -> Result<Vec<String>, String> {
        Ok(self.group_service_ids(group_id)?.into_iter().map(|(sid, _)| sid).collect())
    }

    pub fn preset_ids(&self, preset_id: &str) -> Result<Vec<String>, String> {
        let ws = self.current()?;
        let p = ws.presets.iter().find(|p| p.id == preset_id).ok_or_else(|| format!("프리셋을 찾을 수 없습니다: {preset_id}"))?;
        Ok(p.services.clone())
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
pub async fn service_start(core: Core<'_>, id: String, debug: Option<bool>) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || core.launch(vec![id], debug.unwrap_or(false), true)).await
}

#[tauri::command]
pub async fn service_stop(core: Core<'_>, id: String) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || core.manager.stop(&id)).await
}

#[tauri::command]
pub async fn service_restart(core: Core<'_>, id: String, debug: Option<bool>) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || {
        core.manager.stop(&id)?;
        core.launch(vec![id], debug.unwrap_or(false), true)
    })
    .await
}

#[tauri::command]
pub async fn group_start(core: Core<'_>, id: String) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || {
        let ids = core.group_ids(&id)?;
        core.launch(ids, false, true)
    })
    .await
}

#[tauri::command]
pub async fn preset_start(core: Core<'_>, id: String) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || {
        let ids = core.preset_ids(&id)?;
        core.launch(ids, false, true)
    })
    .await
}

#[tauri::command]
pub async fn preset_stop(core: Core<'_>, id: String) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || {
        let ids = core.preset_ids(&id)?;
        core.stop_ids(&ids)
    })
    .await
}

#[tauri::command]
pub async fn group_stop(core: Core<'_>, id: String) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || {
        let ids = core.group_ids(&id)?;
        core.stop_ids(&ids)
    })
    .await
}

#[tauri::command]
pub async fn stop_all(core: Core<'_>) -> Result<(), String> {
    let core = core.inner().clone();
    blocking(move || {
        // 실행 중인 것과 대기 중인 것 모두
        let mut ids = core.manager.running_ids();
        if let Ok(ws) = core.current() {
            ids.extend(ws.services.iter().map(|s| s.id.clone()).filter(|id| core.manager.is_waiting(id) || core.manager.is_preparing(id)));
        }
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
    let cp = (service.kind == ServiceKind::JavaMain).then(|| core.preview_classpath(&service));
    core.build_spec(&ws, &service, false, cp.as_ref()).map(|s| LaunchPreview::from(&s))
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

/// 설치된 글꼴 목록. 처음 한 번 읽는 데 수십 ms가 걸려 별도 스레드에서 돈다.
#[tauri::command]
pub async fn system_fonts() -> Result<Vec<crate::fonts::FontFamily>, String> {
    crate::fonts::system_fonts()
}

#[tauri::command]
pub fn settings_get(core: Core) -> Settings {
    core.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn settings_save(core: Core, mut settings: Settings) -> Result<Settings, String> {
    settings.fonts = settings.fonts.sanitized();
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

/// 이 폴더 아래에서 실행하는 서비스를 `<폴더>/services-dashboard.json`으로 내보낸다 (팀 공유용).
#[tauri::command]
pub fn project_export(core: Core, root: String) -> Result<String, String> {
    let ws = core.current()?;
    let norm = |p: &str| p.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let root_n = norm(root.trim());
    let services: Vec<Service> = ws
        .services
        .iter()
        .filter(|s| {
            core.expanded_cwd(&ws, s).is_ok_and(|c| {
                let c = norm(&c.to_string_lossy());
                c == root_n || c.starts_with(&format!("{root_n}/"))
            })
        })
        .cloned()
        .collect();
    if services.is_empty() {
        return Err("이 폴더 아래에서 실행하는 서비스가 없습니다.".into());
    }
    let root_values: Vec<String> =
        ws.variables.iter().filter(|(_, v)| norm(v) == root_n).map(|(k, _)| format!("${{var:{k}}}")).collect();
    let path = importers::shared::export(std::path::Path::new(root.trim()), &services, &ws.groups, &ws.presets, &root_values)?;
    Ok(path.to_string_lossy().into_owned())
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

// ---------------------------------------------------------------- git 브랜치

/// 서비스가 쓰는 git 저장소 하나
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GitRepo {
    /// 작업 트리 루트
    pub root: String,
    /// 같은 경로의 워크스페이스 변수 이름, 없으면 폴더 이름
    pub name: String,
    pub branch: Option<String>,
    /// detached HEAD일 때 짧은 커밋 해시
    pub commit: Option<String>,
    /// 이 저장소 안에서 실행하는 서비스
    pub services: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchResult {
    pub branch: String,
    /// 전환하려고 중지한 서비스
    pub stopped: Vec<String>,
    pub restarted: bool,
    /// 전환은 됐지만 pull·다시 시작에서 생긴 문제
    pub warnings: Vec<String>,
}

impl AppCore {
    /// 서비스 작업 디렉터리가 속한 git 저장소 (같은 저장소를 쓰는 서비스끼리 묶는다).
    /// `.git/HEAD`만 읽으므로 자주 불러도 가볍다.
    fn repos(&self) -> Result<Vec<GitRepo>, String> {
        let ws = self.current()?;
        let mut found: std::collections::HashMap<std::path::PathBuf, Option<git::RepoLocation>> = Default::default();
        let mut out: Vec<(String, GitRepo)> = Vec::new();
        for svc in &ws.services {
            let Ok(cwd) = self.expanded_cwd(&ws, svc) else { continue };
            let Some(loc) = found.entry(cwd.clone()).or_insert_with(|| git::find_repo(&cwd)).clone() else { continue };
            let key = git::norm(&loc.root);
            if let Some((_, r)) = out.iter_mut().find(|(k, _)| *k == key) {
                r.services.push(svc.id.clone());
                continue;
            }
            let head = git::read_head(&loc.git_dir);
            let name = ws
                .variables
                .iter()
                .find(|(_, v)| git::norm(std::path::Path::new(v.as_str())) == key)
                .map(|(k, _)| k.clone())
                .or_else(|| loc.root.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| git::display(&loc.root));
            out.push((key, GitRepo { root: git::display(&loc.root), name, branch: head.branch, commit: head.commit, services: vec![svc.id.clone()] }));
        }
        Ok(out.into_iter().map(|(_, r)| r).collect())
    }

    /// 브랜치 전환: 변경 확인 → 이 저장소에서 실행 중인 서비스 중지 → 전환(→ pull) → 중지했던 서비스 다시 시작
    fn switch_branch(self: &Arc<Self>, root: &str, target: &str, pull: bool, restart: bool) -> Result<SwitchResult, String> {
        let repo_git = git::Git::open(std::path::Path::new(root))?;
        // 서비스를 멈추기 전에 확인한다 (전환할 수 없는데 서비스만 내려가지 않도록)
        repo_git.ensure_clean()?;
        let key = git::norm(std::path::Path::new(root));
        let repo = self.repos()?.into_iter().find(|r| git::norm(std::path::Path::new(&r.root)) == key);
        let from = repo.as_ref().map(|r| r.branch.clone().or_else(|| r.commit.clone()).unwrap_or_default()).unwrap_or_default();
        let ids: Vec<String> = repo
            .map(|r| r.services)
            .unwrap_or_default()
            .into_iter()
            .filter(|id| self.is_active(id) || self.manager.status(id).state == RunState::Waiting)
            .collect();
        for id in &ids {
            self.logs.push(id, STREAM_SYS, format!("⎇ 브랜치 전환: {from} → {target}"));
        }
        self.manager.stop_many(&ids)?;
        let branch = match repo_git.switch(target) {
            Ok(b) => b,
            Err(e) => {
                // 전환하지 못했으면 내렸던 서비스를 원래 브랜치 그대로 다시 올린다 (콘솔은 지우지 않는다)
                for id in &ids {
                    self.logs.push(id, STREAM_SYS, format!("✕ 브랜치 전환 실패: {e}"));
                }
                if restart && !ids.is_empty() {
                    let _ = self.launch_opts(ids.clone(), false, true, false);
                }
                return Err(e);
            }
        };
        let mut warnings = Vec::new();
        if pull {
            if let Err(e) = repo_git.pull() {
                warnings.push(format!("pull 실패: {e}"));
            }
        }
        let restarted = restart && !ids.is_empty();
        if restarted {
            // 새 브랜치로 다시 시작: 이전 로그를 지우되 무엇이 바뀌었는지는 첫 줄로 남긴다
            if self.clear_on_start() {
                for id in &ids {
                    self.logs.reset(id);
                    self.logs.push(id, STREAM_SYS, format!("⎇ 브랜치 전환: {from} → {branch}{}", if pull { " (pull)" } else { "" }));
                }
            }
            if let Err(e) = self.launch_opts(ids.clone(), false, true, false) {
                warnings.push(format!("다시 시작 실패: {e}"));
            }
        }
        Ok(SwitchResult { branch, stopped: ids, restarted, warnings })
    }
}

#[tauri::command]
pub fn git_repos(core: Core) -> Result<Vec<GitRepo>, String> {
    core.repos()
}

#[tauri::command]
pub async fn git_branches(root: String) -> Result<git::BranchInfo, String> {
    blocking(move || git::Git::open(std::path::Path::new(&root))?.branches()).await
}

#[tauri::command]
pub async fn git_fetch(root: String) -> Result<(), String> {
    blocking(move || git::Git::open(std::path::Path::new(&root))?.fetch()).await
}

#[tauri::command]
pub async fn git_switch(core: Core<'_>, root: String, branch: String, pull: bool, restart: bool) -> Result<SwitchResult, String> {
    let core = core.inner().clone();
    blocking(move || core.switch_branch(&root, &branch, pull, restart)).await
}
