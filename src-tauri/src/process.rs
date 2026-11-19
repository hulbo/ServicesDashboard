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

/// 줄바꿈 없이 이 시간 동안 출력이 멈추면 남은 내용을 한 줄로 내보낸다.
const PARTIAL_FLUSH: Duration = Duration::from_millis(150);
/// 중지 요청 후 프로세스 종료를 기다리는 최대 시간
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Stopped,
    /// 실행했지만 아직 준비(Ready) 전. M3의 준비 감지에서 쓴다.
    #[allow(dead_code)]
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
        }
    }
}

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
}

impl ProcessManager {
    pub fn new(logs: Arc<LogHub>, sink: Arc<dyn EventSink>) -> Arc<Self> {
        Arc::new(ProcessManager {
            running: Mutex::new(HashMap::new()),
            exited: Condvar::new(),
            statuses: Mutex::new(HashMap::new()),
            logs,
            sink,
            next_run: AtomicU64::new(1),
        })
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

    pub fn start(self: &Arc<Self>, id: &str, spec: LaunchSpec) -> Result<ServiceStatus, String> {
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
        let output_done = self.spawn_output_pump(id, &mut child, spec.encoding);

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
            state: RunState::Running,
            pid: Some(pid),
            exit_code: None,
            started_at: Some(started_at),
            ended_at: None,
            run_id,
            command: Some(spec.display.clone()),
            error: None,
        };
        self.set_status(status.clone());

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
    fn spawn_output_pump(&self, id: &str, child: &mut Child, encoding: OutputEncoding) -> mpsc::Receiver<()> {
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
            let push = |stream: u8, bytes: &[u8]| hub.push(&id, stream, logs::decode(bytes, encoding));
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

        {
            let mut running = self.running.lock().unwrap();
            if running.get(&id).is_some_and(|r| r.run_id == run_id) {
                running.remove(&id);
            }
        }
        self.exited.notify_all();
        drop(job);

        self.set_status(ServiceStatus {
            id,
            state,
            pid: None,
            exit_code,
            started_at: Some(started_at),
            ended_at: Some(ended_at),
            run_id,
            command: Some(display),
            error: None,
        });
    }

    /// 프로세스 트리 전체를 종료하고, 종료될 때까지 기다린다.
    pub fn stop(&self, id: &str) -> Result<(), String> {
        let run_id = {
            let running = self.running.lock().unwrap();
            let Some(r) = running.get(id) else { return Ok(()) };
            r.stop_requested.store(true, Ordering::SeqCst);
            r.job.terminate();
            r.run_id
        };
        let mut status = self.status(id);
        if status.run_id == run_id && status.state == RunState::Running {
            status.state = RunState::Stopping;
            self.set_status(status);
        }
        self.wait_until_gone(&[(id.to_string(), run_id)])
    }

    pub fn stop_many(&self, ids: &[String]) -> Result<(), String> {
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
            if status.run_id == *run_id && status.state == RunState::Running {
                status.state = RunState::Stopping;
                self.set_status(status);
            }
        }
        self.wait_until_gone(&targets)
    }

    /// 앱 종료 시: 기다리지 않고 모든 프로세스 트리를 종료한다.
    pub fn kill_all_now(&self) {
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
        pm.start("t1", shell_spec(cmd)).unwrap();
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
        let started = pm.start("t2", shell_spec(cmd)).unwrap();
        assert_eq!(started.state, RunState::Running);
        thread::sleep(Duration::from_millis(500));
        let t = Instant::now();
        pm.stop("t2").unwrap();
        assert!(t.elapsed() < Duration::from_secs(5), "중지가 너무 오래 걸림: {:?}", t.elapsed());
        let s = wait_state(&pm, "t2", RunState::Stopped);
        assert!(s.pid.is_none());
        assert!(texts(&hub, "t2").last().unwrap().contains("중지됨"));
        // 다시 시작할 수 있다
        pm.start("t2", shell_spec("echo again")).unwrap();
        wait_state(&pm, "t2", RunState::Exited);
    }

    #[test]
    fn missing_program_reports_failure() {
        let hub = Arc::new(LogHub::new(100));
        let pm = ProcessManager::new(hub.clone(), Arc::new(Collect::default()));
        let mut spec = shell_spec("x");
        spec.program = "definitely-not-a-real-program-xyz".into();
        spec.raw_tail = None;
        let err = pm.start("t3", spec).unwrap_err();
        assert!(err.contains("찾을 수 없습니다"));
        assert_eq!(pm.status("t3").state, RunState::Failed);
    }

    #[test]
    fn duration_format() {
        assert_eq!(format_duration(Duration::from_millis(2500)), "2.5초");
        assert_eq!(format_duration(Duration::from_secs(125)), "2분 5초");
        assert_eq!(format_duration(Duration::from_secs(3720)), "1시간 2분");
    }
}
