//! 서비스 프로세스 실행·중지·상태 관리.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::job::{self, Job};
use crate::launch::LaunchSpec;
use crate::logs::{self, LineAssembler, LogBatch, LogHub, STREAM_ERR, STREAM_OUT, STREAM_SYS};
use crate::model::OutputEncoding;
use crate::watch::{self, ReadySpec};

/// 감지한 포트·URL을 이 개수까지만 보관한다.
const MAX_FOUND: usize = 6;

/// 줄바꿈 없이 이 시간 동안 출력이 멈추면 남은 내용을 한 줄로 내보낸다.
const PARTIAL_FLUSH: Duration = Duration::from_millis(150);
/// 중지 요청 후 프로세스 종료를 기다리는 최대 시간
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Stopped,
    /// 의존 서비스가 준비되기를 기다리는 중 (아직 프로세스 없음)
    Waiting,
    /// 실행했지만 아직 준비(Ready) 전 (로그 패턴·포트·HTTP 확인을 기다리는 중)
    Starting,
    Running,
    Stopping,
    /// 스스로 정상 종료 (exit code 0)
    Exited,
    /// 비정상 종료 (exit code ≠ 0) 또는 실행 실패
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub id: String,
    pub state: RunState,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub run_id: u64,
    pub command: Option<String>,
    pub error: Option<String>,
    /// 로그에서 감지한 포트
    pub ports: Vec<u16>,
    /// 로그에서 감지한 로컬 URL
    pub urls: Vec<String>,
    /// 준비 완료 시각
    pub ready_at: Option<u64>,
    /// 디버그로 시작했으면 디버거 포트
    pub debug_port: Option<u16>,
}

impl ServiceStatus {
    fn stopped(id: &str) -> Self {
        ServiceStatus {
            id: id.to_string(),
            state: RunState::Stopped,
            pid: None,
            exit_code: None,
            started_at: None,
            ended_at: None,
            run_id: 0,
            command: None,
            error: None,
            ports: Vec::new(),
            urls: Vec::new(),
            ready_at: None,
            debug_port: None,
        }
    }
}

/// 출력 한 줄을 받는 콜백 (준비 감지·포트 감지)
type LineHook = Arc<dyn Fn(&str) + Send + Sync>;

pub trait EventSink: Send + Sync + 'static {
    fn status(&self, status: &ServiceStatus);
    fn logs(&self, batches: Vec<LogBatch>);
}

struct Running {
    run_id: u64,
    job: Arc<Job>,
    stop_requested: Arc<AtomicBool>,
    _stdin: Option<ChildStdin>,
}

pub struct ProcessManager {
    running: Mutex<HashMap<String, Running>>,
    exited: Condvar,
    statuses: Mutex<HashMap<String, ServiceStatus>>,
    logs: Arc<LogHub>,
    sink: Arc<dyn EventSink>,
    next_run: AtomicU64,
    /// 서비스별로 지금까지 감지한 포트 (다음 시작 전 포트 충돌 검사에 쓴다)
    known_ports: Mutex<HashMap<String, Vec<u16>>>,
    /// java-main 클래스패스 준비(빌드) 중인 프로세스
    preparing: Mutex<HashMap<String, (Arc<Job>, Arc<AtomicBool>)>>,
}

/// 준비 단계가 사용자 중지로 끝났을 때 돌려주는 오류 (실패로 알리지 않는다)
pub const CANCELLED: &str = "__cancelled__";

impl ProcessManager {
    pub fn new(logs: Arc<LogHub>, sink: Arc<dyn EventSink>) -> Arc<Self> {
        Arc::new(ProcessManager {
            running: Mutex::new(HashMap::new()),
            exited: Condvar::new(),
            statuses: Mutex::new(HashMap::new()),
            logs,
            sink,
            next_run: AtomicU64::new(1),
            known_ports: Mutex::new(HashMap::new()),
            preparing: Mutex::new(HashMap::new()),
        })
    }

    pub fn is_preparing(&self, id: &str) -> bool {
        self.preparing.lock().unwrap().contains_key(id)
    }

    /// 의존 서비스를 기다리는 '대기 중'으로 표시한다 (실행 중이면 그대로 둔다).
    pub fn set_waiting(&self, id: &str) {
        if self.is_running(id) || self.is_preparing(id) {
            return;
        }
        let mut st = self.status(id);
        if matches!(st.state, RunState::Starting | RunState::Running | RunState::Stopping) {
            return;
        }
        st.state = RunState::Waiting;
        st.pid = None;
        st.error = None;
        st.exit_code = None;
        self.set_status(st);
    }

    pub fn is_waiting(&self, id: &str) -> bool {
        self.status(id).state == RunState::Waiting
    }

    /// 대기 중인 서비스를 시작하지 못하게 됐을 때
    pub fn fail_waiting(&self, id: &str, msg: &str) {
        self.fail_waiting_with(id, msg, true)
    }

    /// `log`가 false면 콘솔에 안내 줄을 남기지 않는다 (이미 남긴 경우)
    pub fn fail_waiting_with(&self, id: &str, msg: &str, log: bool) {
        let mut st = self.status(id);
        if st.state != RunState::Waiting {
            return;
        }
        if log {
            self.logs.push(id, STREAM_SYS, format!("✕ {msg}"));
        }
        st.state = RunState::Failed;
        st.error = Some(msg.to_string());
        st.ended_at = Some(logs::now_millis());
        self.set_status(st);
    }

    /// 대기 중이면 취소한다. 취소했으면 true.
    pub fn cancel_waiting(&self, id: &str) -> bool {
        let mut st = self.status(id);
        if st.state != RunState::Waiting {
            return false;
        }
        st.state = RunState::Stopped;
        self.set_status(st);
        true
    }

    /// 실행 전에 필요한 작업(java-main 컴파일·클래스패스 계산)을 돌리고 끝날 때까지 기다린다.
    /// 출력은 서비스 콘솔에 그대로 보인다. 중지하면 `CANCELLED`를 돌려준다.
    pub fn run_prepare(&self, id: &str, spec: LaunchSpec) -> Result<(), String> {
        if self.is_running(id) || self.is_preparing(id) {
            return Err("이미 실행 중입니다.".into());
        }
        self.logs.push(id, STREAM_SYS, format!("▶ 준비: {}", spec.display));
        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(spec.env.iter())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        if let Some(tail) = &spec.raw_tail {
            use std::os::windows::process::CommandExt;
            cmd.raw_arg(tail);
        }
        job::configure(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| spawn_error_message(&spec.program, &e))?;
        let job = Arc::new(Job::new().map_err(|e| format!("Job Object를 만들 수 없습니다: {e}"))?);
        let _ = job.assign(&child);
        let cancelled = Arc::new(AtomicBool::new(false));
        self.preparing.lock().unwrap().insert(id.to_string(), (job.clone(), cancelled.clone()));
        let mut st = self.status(id);
        st.state = RunState::Starting;
        st.pid = Some(child.id());
        st.command = Some(spec.display.clone());
        st.error = None;
        self.set_status(st);

        let done = self.spawn_output_pump(id, &mut child, spec.encoding, Arc::new(|_: &str| {}));
        let code = child.wait().ok().and_then(|s| s.code());
        let _ = done.recv_timeout(Duration::from_secs(3));
        job.terminate();
        self.preparing.lock().unwrap().remove(id);

        if cancelled.load(Ordering::SeqCst) {
            self.logs.push(id, STREAM_SYS, "■ 준비 중에 중지됨".to_string());
            let mut st = self.status(id);
            st.state = RunState::Stopped;
            st.pid = None;
            self.set_status(st);
            return Err(CANCELLED.into());
        }
        if code != Some(0) {
            let msg = format!("준비(빌드) 단계가 실패했습니다 (종료 코드 {}).", code.map(|c| c.to_string()).unwrap_or_else(|| "?".into()));
            self.logs.push(id, STREAM_SYS, format!("✕ {msg}"));
            let mut st = self.status(id);
            st.state = RunState::Failed;
            st.pid = None;
            st.exit_code = code;
            st.error = Some(msg.clone());
            st.ended_at = Some(logs::now_millis());
            self.set_status(st);
            return Err(msg);
        }
        Ok(())
    }

    pub fn known_ports(&self, id: &str) -> Vec<u16> {
        self.known_ports.lock().unwrap().get(id).cloned().unwrap_or_default()
    }

    /// 이 PID가 실행 중인 서비스의 프로세스 트리에 속하면 그 서비스 ID
    pub fn service_owning(&self, pid: u32) -> Option<String> {
        self.running.lock().unwrap().iter().find(|(_, r)| r.job.contains(pid)).map(|(id, _)| id.clone())
    }

    pub fn statuses(&self) -> Vec<ServiceStatus> {
        self.statuses.lock().unwrap().values().cloned().collect()
    }

    pub fn status(&self, id: &str) -> ServiceStatus {
        self.statuses.lock().unwrap().get(id).cloned().unwrap_or_else(|| ServiceStatus::stopped(id))
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.running.lock().unwrap().contains_key(id)
    }

    pub fn running_ids(&self) -> Vec<String> {
        self.running.lock().unwrap().keys().cloned().collect()
    }

    fn set_status(&self, status: ServiceStatus) {
        self.statuses.lock().unwrap().insert(status.id.clone(), status.clone());
        self.sink.status(&status);
    }

    /// `ready`가 있으면 '시작 중'으로 두었다가 준비 신호를 받으면 '실행 중'으로 바꾼다.
    pub fn start(self: &Arc<Self>, id: &str, spec: LaunchSpec, ready: Option<ReadySpec>) -> Result<ServiceStatus, String> {
        if self.is_running(id) {
            return Err("이미 실행 중입니다.".into());
        }
        let run_id = self.next_run.fetch_add(1, Ordering::SeqCst);
        self.logs.push(id, STREAM_SYS, format!("▶ {}", spec.display));
        self.logs.push(id, STREAM_SYS, format!("  작업 디렉터리: {}", spec.cwd.display()));

        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(spec.env.iter())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        if let Some(tail) = &spec.raw_tail {
            use std::os::windows::process::CommandExt;
            cmd.raw_arg(tail);
        }
        job::configure(&mut cmd);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let msg = spawn_error_message(&spec.program, &e);
                self.logs.push(id, STREAM_SYS, format!("✕ {msg}"));
                let status = ServiceStatus {
                    state: RunState::Failed,
                    run_id,
                    command: Some(spec.display.clone()),
                    error: Some(msg.clone()),
                    ended_at: Some(logs::now_millis()),
                    ..ServiceStatus::stopped(id)
                };
                self.set_status(status);
                return Err(msg);
            }
        };

        let job = Arc::new(Job::new().map_err(|e| format!("Job Object를 만들 수 없습니다: {e}"))?);
        if let Err(e) = job.assign(&child) {
            self.logs.push(id, STREAM_SYS, format!("⚠ 프로세스 트리 묶기에 실패했습니다. 중지 시 하위 프로세스가 남을 수 있습니다: {e}"));
        }

        let pid = child.id();
        let started_at = logs::now_millis();
        let stop_requested = Arc::new(AtomicBool::new(false));

        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(id) {
                // 동시에 두 번 시작된 경우: 나중 것을 정리한다.
                job.terminate();
                return Err("이미 실행 중입니다.".into());
            }
            running.insert(
                id.to_string(),
                Running { run_id, job: job.clone(), stop_requested: stop_requested.clone(), _stdin: child.stdin.take() },
            );
        }
        let status = ServiceStatus {
            id: id.to_string(),
            state: if ready.is_some() { RunState::Starting } else { RunState::Running },
            pid: Some(pid),
            exit_code: None,
            started_at: Some(started_at),
            ended_at: None,
            run_id,
            command: Some(spec.display.clone()),
            error: None,
            ports: Vec::new(),
            urls: Vec::new(),
            ready_at: if ready.is_some() { None } else { Some(started_at) },
            debug_port: spec.debug_port,
        };
        if let Some(p) = spec.debug_port {
            self.logs.push(id, STREAM_SYS, format!("🐞 디버거 포트 {p} — IDE에서 Remote JVM Debug / Attach로 연결하세요."));
        }
        // 상태를 먼저 등록해야 출력 첫 줄부터 준비 신호를 놓치지 않는다.
        self.set_status(status.clone());

        let watcher = {
            let me = Arc::clone(self);
            let id = id.to_string();
            let ready = ready.clone();
            Arc::new(move |text: &str| me.on_line(&id, run_id, ready.as_ref(), text)) as LineHook
        };
        let output_done = self.spawn_output_pump(id, &mut child, spec.encoding, watcher);
        if let Some(r) = ready {
            self.spawn_ready_probe(id, run_id, r);
        }

        let me = Arc::clone(self);
        let id_owned = id.to_string();
        let display = spec.display.clone();
        thread::Builder::new()
            .name(format!("wait-{id}"))
            .spawn(move || me.wait_for_exit(id_owned, run_id, child, job, stop_requested, output_done, started_at, display))
            .map_err(|e| format!("감시 스레드를 시작할 수 없습니다: {e}"))?;
        Ok(status)
    }

    /// stdout/stderr를 읽어 줄 단위로 LogHub에 넣는다. 모든 출력이 끝나면 반환한 채널로 알린다.
    fn spawn_output_pump(&self, id: &str, child: &mut Child, encoding: OutputEncoding, hook: LineHook) -> mpsc::Receiver<()> {
        enum Msg {
            Chunk(u8, Vec<u8>),
            Eof,
        }
        let (tx, rx) = mpsc::channel::<Msg>();
        let mut readers = 0;
        let streams: [(u8, Option<Box<dyn Read + Send>>); 2] = [
            (STREAM_OUT, child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>)),
            (STREAM_ERR, child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>)),
        ];
        for (stream, reader) in streams {
            let Some(mut reader) = reader else { continue };
            readers += 1;
            let tx = tx.clone();
            let _ = thread::Builder::new().name(format!("read-{id}-{stream}")).spawn(move || {
                let mut buf = vec![0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(Msg::Chunk(stream, buf[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                    }
                }
                let _ = tx.send(Msg::Eof);
            });
        }
        drop(tx);

        let (done_tx, done_rx) = mpsc::channel();
        let hub = Arc::clone(&self.logs);
        let id = id.to_string();
        let _ = thread::Builder::new().name(format!("lines-{id}")).spawn(move || {
            let mut asm = [LineAssembler::default(), LineAssembler::default()];
            let mut open = readers;
            let push = |stream: u8, bytes: &[u8]| {
                let text = logs::decode(bytes, encoding);
                hook(&text);
                hub.push(&id, stream, text);
            };
            while open > 0 {
                match rx.recv_timeout(PARTIAL_FLUSH) {
                    Ok(Msg::Chunk(stream, bytes)) => {
                        asm[stream as usize].feed(&bytes, |l| push(stream, l));
                    }
                    Ok(Msg::Eof) => open -= 1,
                    Err(RecvTimeoutError::Timeout) => {
                        for (i, a) in asm.iter_mut().enumerate() {
                            if a.has_pending() {
                                a.flush(|l| push(i as u8, l));
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            for (i, a) in asm.iter_mut().enumerate() {
                a.flush(|l| push(i as u8, l));
            }
            let _ = done_tx.send(());
        });
        done_rx
    }

    fn is_starting(&self, id: &str, run_id: u64) -> bool {
        self.statuses.lock().unwrap().get(id).is_some_and(|s| s.run_id == run_id && s.state == RunState::Starting)
    }

    /// 출력 한 줄마다: 준비 신호 확인, 포트·URL 감지
    fn on_line(&self, id: &str, run_id: u64, ready: Option<&ReadySpec>, text: &str) {
        let plain = watch::strip_ansi(text);
        if let Some(spec) = ready {
            if spec.matches(&plain) && self.is_starting(id, run_id) {
                self.mark_ready(id, run_id, None);
            }
        }
        let found = watch::detect(&plain);
        if found.ports.is_empty() && found.urls.is_empty() {
            return;
        }
        {
            let mut known = self.known_ports.lock().unwrap();
            let list = known.entry(id.to_string()).or_default();
            for p in &found.ports {
                if !list.contains(p) {
                    list.push(*p);
                }
            }
        }
        let updated = {
            let mut statuses = self.statuses.lock().unwrap();
            let Some(st) = statuses.get_mut(id).filter(|s| s.run_id == run_id) else { return };
            let mut changed = false;
            for p in found.ports {
                if !st.ports.contains(&p) && st.ports.len() < MAX_FOUND {
                    st.ports.push(p);
                    changed = true;
                }
            }
            for u in found.urls {
                if !st.urls.contains(&u) && st.urls.len() < MAX_FOUND {
                    st.urls.push(u);
                    changed = true;
                }
            }
            changed.then(|| st.clone())
        };
        if let Some(st) = updated {
            self.sink.status(&st);
        }
    }

    fn mark_ready(&self, id: &str, run_id: u64, note: Option<String>) {
        let updated = {
            let mut statuses = self.statuses.lock().unwrap();
            let Some(st) = statuses.get_mut(id).filter(|s| s.run_id == run_id && s.state == RunState::Starting) else {
                return;
            };
            st.state = RunState::Running;
            st.ready_at = Some(logs::now_millis());
            st.clone()
        };
        let took = Duration::from_millis(updated.ready_at.unwrap_or(0).saturating_sub(updated.started_at.unwrap_or(0)));
        let msg = note.unwrap_or_else(|| format!("✓ 준비 완료 ({})", format_duration(took)));
        self.logs.push(id, STREAM_SYS, msg);
        self.sink.status(&updated);
    }

    /// 포트·HTTP 확인과 제한 시간 처리 (준비될 때까지 1초마다)
    fn spawn_ready_probe(self: &Arc<Self>, id: &str, run_id: u64, spec: ReadySpec) {
        let me = Arc::clone(self);
        let id = id.to_string();
        let started = Instant::now();
        let _ = thread::Builder::new().name(format!("ready-{id}")).spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(if spec.needs_probe() { 700 } else { 1000 }));
                if !me.is_starting(&id, run_id) {
                    return;
                }
                if let Some(p) = spec.ports.iter().find(|p| watch::port_open(**p)) {
                    me.mark_ready(&id, run_id, Some(format!("✓ 준비 완료 (포트 {p} 응답, {})", format_duration(started.elapsed()))));
                    return;
                }
                if let Some(url) = spec.http_url.as_deref().filter(|u| watch::http_ok(u)) {
                    me.mark_ready(&id, run_id, Some(format!("✓ 준비 완료 ({url} 응답, {})", format_duration(started.elapsed()))));
                    return;
                }
                if started.elapsed() >= spec.timeout {
                    me.mark_ready(
                        &id,
                        run_id,
                        Some(format!(
                            "⚠ {}초 동안 준비 신호를 찾지 못해 '실행 중'으로 표시합니다. 설정의 준비 감지를 확인하세요.",
                            spec.timeout.as_secs()
                        )),
                    );
                    return;
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn wait_for_exit(
        self: Arc<Self>,
        id: String,
        run_id: u64,
        mut child: Child,
        job: Arc<Job>,
        stop_requested: Arc<AtomicBool>,
        output_done: mpsc::Receiver<()>,
        started_at: u64,
        display: String,
    ) {
        let exit_code = child.wait().ok().and_then(|s| s.code());
        // 남은 출력을 잠시 기다린 뒤, 남아 있는 하위 프로세스를 정리한다 (파이프가 닫혀 출력도 끝난다).
        if output_done.recv_timeout(Duration::from_millis(1500)).is_err() {
            job.terminate();
            let _ = output_done.recv_timeout(Duration::from_millis(1500));
        } else {
            job.terminate();
        }

        let stopped = stop_requested.load(Ordering::SeqCst);
        let ended_at = logs::now_millis();
        let elapsed = format_duration(Duration::from_millis(ended_at.saturating_sub(started_at)));
        let state = if stopped {
            RunState::Stopped
        } else if exit_code == Some(0) {
            RunState::Exited
        } else {
            RunState::Failed
        };
        let code_text = exit_code.map(|c| c.to_string()).unwrap_or_else(|| "알 수 없음".into());
        let msg = match state {
            RunState::Stopped => format!("■ 중지됨 (실행 시간 {elapsed})"),
            RunState::Exited => format!("■ 종료됨 (종료 코드 0, 실행 시간 {elapsed})"),
            _ => format!("✕ 비정상 종료 (종료 코드 {code_text}, 실행 시간 {elapsed})"),
        };
        self.logs.push(&id, STREAM_SYS, msg);

        let (ports, urls, ready_at, debug_port) = {
            let statuses = self.statuses.lock().unwrap();
            statuses
                .get(&id)
                .filter(|s| s.run_id == run_id)
                .map(|s| (s.ports.clone(), s.urls.clone(), s.ready_at, s.debug_port))
                .unwrap_or_default()
        };
        self.set_status(ServiceStatus {
            id: id.clone(),
            state,
            pid: None,
            exit_code,
            started_at: Some(started_at),
            ended_at: Some(ended_at),
            run_id,
            command: Some(display),
            error: None,
            ports,
            urls,
            ready_at,
            debug_port,
        });

        // 최종 상태를 먼저 기록한 뒤에 실행 목록에서 뺀다. 순서가 반대면 stop()이 돌아온 직후
        // (재시작 등) 상태가 아직 '중지 중'으로 보여 다시 시작하지 않거나, 늦게 온 '중지됨'이 새 실행의 '대기 중'을 덮어쓴다.
        {
            let mut running = self.running.lock().unwrap();
            if running.get(&id).is_some_and(|r| r.run_id == run_id) {
                running.remove(&id);
            }
        }
        self.exited.notify_all();
        drop(job);
    }

    /// 프로세스 트리 전체를 종료하고, 종료될 때까지 기다린다.
    pub fn stop(&self, id: &str) -> Result<(), String> {
        if self.cancel_waiting(id) {
            return Ok(());
        }
        if let Some((job, cancelled)) = self.preparing.lock().unwrap().get(id).cloned() {
            cancelled.store(true, Ordering::SeqCst);
            job.terminate();
            return Ok(());
        }
        let run_id = {
            let running = self.running.lock().unwrap();
            let Some(r) = running.get(id) else { return Ok(()) };
            r.stop_requested.store(true, Ordering::SeqCst);
            r.job.terminate();
            r.run_id
        };
        let mut status = self.status(id);
        if status.run_id == run_id && matches!(status.state, RunState::Running | RunState::Starting) {
            status.state = RunState::Stopping;
            self.set_status(status);
        }
        self.wait_until_gone(&[(id.to_string(), run_id)])
    }

    pub fn stop_many(&self, ids: &[String]) -> Result<(), String> {
        for id in ids {
            if !self.cancel_waiting(id) {
                if let Some((job, cancelled)) = self.preparing.lock().unwrap().get(id).cloned() {
                    cancelled.store(true, Ordering::SeqCst);
                    job.terminate();
                }
            }
        }
        let mut targets = Vec::new();
        {
            let running = self.running.lock().unwrap();
            for id in ids {
                if let Some(r) = running.get(id) {
                    r.stop_requested.store(true, Ordering::SeqCst);
                    r.job.terminate();
                    targets.push((id.clone(), r.run_id));
                }
            }
        }
        for (id, run_id) in &targets {
            let mut status = self.status(id);
            if status.run_id == *run_id && matches!(status.state, RunState::Running | RunState::Starting) {
                status.state = RunState::Stopping;
                self.set_status(status);
            }
        }
        self.wait_until_gone(&targets)
    }

    /// 앱 종료 시: 기다리지 않고 모든 프로세스 트리를 종료한다.
    pub fn kill_all_now(&self) {
        for (job, cancelled) in self.preparing.lock().unwrap().values() {
            cancelled.store(true, Ordering::SeqCst);
            job.terminate();
        }
        let running = self.running.lock().unwrap();
        for r in running.values() {
            r.stop_requested.store(true, Ordering::SeqCst);
            r.job.terminate();
        }
    }

    fn wait_until_gone(&self, targets: &[(String, u64)]) -> Result<(), String> {
        let deadline = Instant::now() + STOP_TIMEOUT;
        let mut running = self.running.lock().unwrap();
        loop {
            let alive: Vec<&String> = targets
                .iter()
                .filter(|(id, run_id)| running.get(id).is_some_and(|r| r.run_id == *run_id))
                .map(|(id, _)| id)
                .collect();
            if alive.is_empty() {
                return Ok(());
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(format!("종료를 기다리는 중 시간이 초과됐습니다: {}", alive.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
            }
            running = self.exited.wait_timeout(running, deadline - now).unwrap().0;
        }
    }
}

fn spawn_error_message(program: &str, e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => format!(
            "실행 파일을 찾을 수 없습니다: {program}. PATH 또는 런타임(JDK/Node) 경로를 확인하세요."
        ),
        std::io::ErrorKind::InvalidInput => format!(
            "명령 인자에 배치 파일(.bat/.cmd)로 안전하게 전달할 수 없는 문자가 있습니다 ({program}): {e}"
        ),
        _ => format!("프로세스를 시작할 수 없습니다 ({program}): {e}"),
    }
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}.{}초", secs, d.subsec_millis() / 100)
    } else if secs < 3600 {
        format!("{}분 {}초", secs / 60, secs % 60)
    } else {
        format!("{}시간 {}분", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EnvMap;

    #[derive(Default)]
    struct Collect {
        statuses: Mutex<Vec<ServiceStatus>>,
    }
    impl EventSink for Collect {
        fn status(&self, s: &ServiceStatus) {
            self.statuses.lock().unwrap().push(s.clone());
        }
        fn logs(&self, _: Vec<LogBatch>) {}
    }

    fn shell_spec(command: &str) -> LaunchSpec {
        let (program, args, raw_tail) = if cfg!(windows) {
            ("cmd.exe".to_string(), vec!["/d".into(), "/s".into(), "/c".into()], Some(format!("\"{command}\"")))
        } else {
            ("sh".to_string(), vec!["-c".into(), command.to_string()], None)
        };
        LaunchSpec {
            program,
            args,
            raw_tail,
            cwd: std::env::temp_dir(),
            env: EnvMap::from([("SD_TEST_VALUE".to_string(), "hello-env".to_string())]),
            display: command.to_string(),
            encoding: OutputEncoding::Auto,
            debug_port: None,
        }
    }

    fn wait_state(pm: &ProcessManager, id: &str, want: RunState) -> ServiceStatus {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let s = pm.status(id);
            if s.state == want {
                return s;
            }
            assert!(Instant::now() < deadline, "상태 {want:?} 대기 시간 초과 (현재 {:?})", s.state);
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn texts(hub: &LogHub, id: &str) -> Vec<String> {
        hub.snapshot(id).into_iter().map(|l| l.text).collect()
    }

    #[test]
    fn captures_output_env_and_exit_code() {
        let hub = Arc::new(LogHub::new(1000));
        let sink = Arc::new(Collect::default());
        let pm = ProcessManager::new(hub.clone(), sink.clone());
        let cmd = if cfg!(windows) {
            "echo out-line & echo %SD_TEST_VALUE% & echo err-line 1>&2 & exit 3"
        } else {
            "echo out-line; echo $SD_TEST_VALUE; echo err-line 1>&2; exit 3"
        };
        pm.start("t1", shell_spec(cmd), None).unwrap();
        let s = wait_state(&pm, "t1", RunState::Failed);
        assert_eq!(s.exit_code, Some(3));
        let lines = hub.snapshot("t1");
        assert!(lines.iter().any(|l| l.stream == STREAM_OUT && l.text.trim() == "out-line"));
        assert!(lines.iter().any(|l| l.text.trim() == "hello-env"));
        assert!(lines.iter().any(|l| l.stream == STREAM_ERR && l.text.trim() == "err-line"));
        assert!(texts(&hub, "t1").last().unwrap().contains("종료 코드 3"));
        assert!(!pm.is_running("t1"));
    }

    #[test]
    fn stop_kills_whole_tree() {
        let hub = Arc::new(LogHub::new(1000));
        let pm = ProcessManager::new(hub.clone(), Arc::new(Collect::default()));
        // 자식 프로세스(ping/sleep)를 띄운 뒤 오래 기다리는 명령
        let cmd = if cfg!(windows) { "ping -n 60 127.0.0.1 > nul" } else { "sleep 60" };
        let started = pm.start("t2", shell_spec(cmd), None).unwrap();
        assert_eq!(started.state, RunState::Running);
        thread::sleep(Duration::from_millis(500));
        let t = Instant::now();
        pm.stop("t2").unwrap();
        assert!(t.elapsed() < Duration::from_secs(5), "중지가 너무 오래 걸림: {:?}", t.elapsed());
        // stop()이 돌아온 순간 이미 '중지됨'이어야 한다 (재시작이 바로 이어서 시작할 수 있게)
        assert_eq!(pm.status("t2").state, RunState::Stopped, "stop() 직후 상태");
        assert!(!pm.is_running("t2"));
        let s = wait_state(&pm, "t2", RunState::Stopped);
        assert!(s.pid.is_none());
        assert!(texts(&hub, "t2").last().unwrap().contains("중지됨"));
        // 다시 시작할 수 있다
        pm.start("t2", shell_spec("echo again"), None).unwrap();
        wait_state(&pm, "t2", RunState::Exited);
    }

    #[test]
    fn missing_program_reports_failure() {
        let hub = Arc::new(LogHub::new(100));
        let pm = ProcessManager::new(hub.clone(), Arc::new(Collect::default()));
        let mut spec = shell_spec("x");
        spec.program = "definitely-not-a-real-program-xyz".into();
        spec.raw_tail = None;
        let err = pm.start("t3", spec, None).unwrap_err();
        assert!(err.contains("찾을 수 없습니다"));
        assert_eq!(pm.status("t3").state, RunState::Failed);
    }

    fn ready(patterns: &[&str], ports: Vec<u16>, timeout_ms: u64) -> Option<ReadySpec> {
        Some(ReadySpec {
            patterns: patterns.iter().map(|p| regex::Regex::new(p).unwrap()).collect(),
            ports,
            http_url: None,
            timeout: Duration::from_millis(timeout_ms),
        })
    }

    #[test]
    fn ready_by_log_pattern_and_detects_urls() {
        let hub = Arc::new(LogHub::new(1000));
        let pm = ProcessManager::new(hub.clone(), Arc::new(Collect::default()));
        let cmd = if cfg!(windows) {
            "echo booting & ping -n 2 127.0.0.1 > nul & echo Local: http://localhost:5173/ & echo Tomcat started on port 8080 (http) with context path '/api' & ping -n 30 127.0.0.1 > nul"
        } else {
            "echo booting; sleep 1; echo Local: http://localhost:5173/; echo \"Tomcat started on port 8080 (http) with context path '/api'\"; sleep 30"
        };
        let st = pm.start("r1", shell_spec(cmd), ready(&[r"Local:\s+https?://"], vec![], 60_000)).unwrap();
        assert_eq!(st.state, RunState::Starting);
        let s = wait_state(&pm, "r1", RunState::Running);
        assert!(s.ready_at.is_some());
        thread::sleep(Duration::from_millis(300));
        let s = pm.status("r1");
        assert_eq!(s.ports, vec![5173, 8080]);
        assert_eq!(s.urls, vec!["http://localhost:5173/", "http://localhost:8080/api/"]);
        assert!(texts(&hub, "r1").iter().any(|t| t.starts_with("✓ 준비 완료")));
        assert_eq!(pm.known_ports("r1"), vec![5173, 8080]);
        pm.stop("r1").unwrap();
        // 종료 뒤에도 감지한 포트는 남는다 (다음 시작 전 충돌 검사용)
        let s = wait_state(&pm, "r1", RunState::Stopped);
        assert_eq!(s.ports, vec![5173, 8080]);
    }

    #[test]
    fn ready_by_port_and_timeout() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hub = Arc::new(LogHub::new(1000));
        let pm = ProcessManager::new(hub.clone(), Arc::new(Collect::default()));
        let long = if cfg!(windows) { "ping -n 30 127.0.0.1 > nul" } else { "sleep 30" };
        pm.start("p1", shell_spec(long), ready(&[], vec![port], 60_000)).unwrap();
        wait_state(&pm, "p1", RunState::Running);
        assert!(texts(&hub, "p1").iter().any(|t| t.contains(&format!("포트 {port} 응답"))));
        pm.stop("p1").unwrap();

        pm.start("p2", shell_spec(long), ready(&["never-matches"], vec![], 1500)).unwrap();
        assert_eq!(pm.status("p2").state, RunState::Starting);
        wait_state(&pm, "p2", RunState::Running);
        assert!(texts(&hub, "p2").iter().any(|t| t.contains("준비 신호를 찾지 못해")));
        pm.stop("p2").unwrap();
        drop(listener);
    }

    #[test]
    fn waiting_and_prepare() {
        let hub = Arc::new(LogHub::new(1000));
        let pm = ProcessManager::new(hub.clone(), Arc::new(Collect::default()));
        pm.set_waiting("w");
        assert_eq!(pm.status("w").state, RunState::Waiting);
        pm.stop("w").unwrap();
        assert_eq!(pm.status("w").state, RunState::Stopped);
        pm.set_waiting("w");
        pm.fail_waiting("w", "의존 서비스 실패");
        assert_eq!(pm.status("w").state, RunState::Failed);

        // 준비 단계: 성공·실패
        let ok = if cfg!(windows) { "echo compiling" } else { "echo compiling" };
        pm.run_prepare("j", shell_spec(ok)).unwrap();
        assert!(texts(&hub, "j").iter().any(|t| t.trim() == "compiling"));
        let bad = "exit 4";
        assert!(pm.run_prepare("j", shell_spec(bad)).unwrap_err().contains("종료 코드 4"));
        assert_eq!(pm.status("j").state, RunState::Failed);

        // 준비 중 중지 → CANCELLED
        let pm2 = pm.clone();
        let long = if cfg!(windows) { "ping -n 30 127.0.0.1 > nul" } else { "sleep 30" };
        let h = thread::spawn(move || pm2.run_prepare("k", shell_spec(long)));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !pm.is_preparing("k") && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        pm.stop("k").unwrap();
        assert_eq!(h.join().unwrap().unwrap_err(), CANCELLED);
        assert_eq!(pm.status("k").state, RunState::Stopped);
    }

    #[test]
    fn duration_format() {
        assert_eq!(format_duration(Duration::from_millis(2500)), "2.5초");
        assert_eq!(format_duration(Duration::from_secs(125)), "2분 5초");
        assert_eq!(format_duration(Duration::from_secs(3720)), "1시간 2분");
    }
}
