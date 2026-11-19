//! 로그 파일 저장: `<설정 폴더>/logs/<서비스 ID>/<YYYY-MM-DD>.log`

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use chrono::{Local, TimeZone};

use crate::logs::{LogLine, STREAM_ERR, STREAM_SYS};
use crate::watch::strip_ansi;

const FLUSH_EVERY: Duration = Duration::from_millis(1000);

struct Writer {
    date: String,
    out: BufWriter<File>,
}

pub struct LogFiles {
    dir: PathBuf,
    enabled: AtomicBool,
    retention_days: AtomicU32,
    writers: Mutex<HashMap<String, Writer>>,
    last_flush: Mutex<Instant>,
}

/// 파일 이름으로 쓸 수 있게 서비스 ID를 정리한다.
fn safe(id: &str) -> String {
    id.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

pub fn format_line(line: &LogLine) -> String {
    let time = Local
        .timestamp_millis_opt(line.ts as i64)
        .single()
        .map(|t| t.format("%H:%M:%S%.3f").to_string())
        .unwrap_or_default();
    let tag = match line.stream {
        STREAM_ERR => "ERR",
        STREAM_SYS => "SYS",
        _ => "OUT",
    };
    format!("{time} {tag} {}", strip_ansi(&line.text))
}

impl LogFiles {
    pub fn new(dir: PathBuf, enabled: bool, retention_days: u32) -> Self {
        LogFiles {
            dir,
            enabled: AtomicBool::new(enabled),
            retention_days: AtomicU32::new(retention_days),
            writers: Mutex::new(HashMap::new()),
            last_flush: Mutex::new(Instant::now()),
        }
    }

    pub fn configure(&self, enabled: bool, retention_days: u32) {
        self.enabled.store(enabled, Ordering::SeqCst);
        self.retention_days.store(retention_days, Ordering::SeqCst);
        if !enabled {
            self.flush();
            self.writers.lock().unwrap().clear();
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn service_dir(&self, id: &str) -> PathBuf {
        self.dir.join(safe(id))
    }

    pub fn write(&self, id: &str, line: &LogLine) {
        if !self.enabled() {
            return;
        }
        let date = Local::now().format("%Y-%m-%d").to_string();
        let mut writers = self.writers.lock().unwrap();
        let needs_open = writers.get(id).is_none_or(|w| w.date != date);
        if needs_open {
            let dir = self.service_dir(id);
            if fs::create_dir_all(&dir).is_err() {
                return;
            }
            let Ok(file) = OpenOptions::new().create(true).append(true).open(dir.join(format!("{date}.log"))) else {
                return;
            };
            if let Some(mut old) = writers.insert(id.to_string(), Writer { date, out: BufWriter::new(file) }) {
                let _ = old.out.flush();
            }
        }
        if let Some(w) = writers.get_mut(id) {
            let _ = writeln!(w.out, "{}", format_line(line));
        }
    }

    pub fn flush(&self) {
        for w in self.writers.lock().unwrap().values_mut() {
            let _ = w.out.flush();
        }
    }

    /// 마지막 flush 뒤 1초가 지났으면 flush (로그 전송 주기마다 부른다)
    pub fn flush_if_due(&self) {
        let mut last = self.last_flush.lock().unwrap();
        if last.elapsed() >= FLUSH_EVERY {
            *last = Instant::now();
            drop(last);
            self.flush();
        }
    }

    /// 보관 기간이 지난 로그 파일을 지운다. 지운 파일 수를 돌려준다.
    pub fn cleanup(&self) -> usize {
        let days = self.retention_days.load(Ordering::SeqCst);
        if days == 0 {
            return 0;
        }
        cleanup_dir(&self.dir, Duration::from_secs(days as u64 * 86_400))
    }
}

fn cleanup_dir(dir: &Path, keep: Duration) -> usize {
    let mut removed = 0;
    let now = SystemTime::now();
    let Ok(services) = fs::read_dir(dir) else { return 0 };
    for svc in services.flatten().filter(|e| e.path().is_dir()) {
        let Ok(files) = fs::read_dir(svc.path()) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().is_none_or(|e| e != "log") {
                continue;
            }
            let old = f.metadata().and_then(|m| m.modified()).ok().and_then(|m| now.duration_since(m).ok()).is_some_and(|age| age > keep);
            if old && fs::remove_file(&p).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// 콘솔 버퍼를 파일로 내보낸다.
pub fn export(lines: &[LogLine], path: &Path) -> Result<usize, String> {
    let file = File::create(path).map_err(|e| format!("파일을 만들 수 없습니다: {} ({e})", path.display()))?;
    let mut out = BufWriter::new(file);
    for l in lines {
        writeln!(out, "{}", format_line(l)).map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(lines.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logs::STREAM_OUT;

    fn line(seq: u64, stream: u8, text: &str) -> LogLine {
        LogLine { seq, ts: crate::logs::now_millis(), stream, text: text.into() }
    }

    #[test]
    fn writes_daily_files_without_ansi() {
        let dir = tempfile::tempdir().unwrap();
        let files = LogFiles::new(dir.path().to_path_buf(), false, 7);
        files.write("svc/1", &line(1, STREAM_OUT, "꺼져 있을 때는 쓰지 않음"));
        assert!(!dir.path().join("svc_1").exists());
        files.configure(true, 7);
        files.write("svc/1", &line(2, STREAM_OUT, "\x1b[32mINFO\x1b[0m 한글 로그"));
        files.write("svc/1", &line(3, STREAM_ERR, "boom"));
        files.flush();
        let today = Local::now().format("%Y-%m-%d").to_string();
        let text = fs::read_to_string(dir.path().join("svc_1").join(format!("{today}.log"))).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with(" OUT INFO 한글 로그"), "{}", lines[0]);
        assert!(lines[1].ends_with(" ERR boom"));
    }

    #[test]
    fn export_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.log");
        assert_eq!(export(&[line(1, STREAM_SYS, "▶ start"), line(2, STREAM_OUT, "x")], &out).unwrap(), 2);
        assert!(fs::read_to_string(&out).unwrap().contains(" SYS ▶ start"));

        let svc = dir.path().join("logs").join("a");
        fs::create_dir_all(&svc).unwrap();
        let old = svc.join("2000-01-01.log");
        fs::write(&old, "old").unwrap();
        let f = File::options().write(true).open(&old).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(10 * 86_400)).unwrap();
        fs::write(svc.join("today.log"), "new").unwrap();
        let files = LogFiles::new(dir.path().join("logs"), true, 7);
        assert_eq!(files.cleanup(), 1);
        assert!(!old.exists() && svc.join("today.log").exists());
    }
}
