//! 서비스별 로그 링 버퍼와 줄 조립·디코딩.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use encoding_rs::EUC_KR;
use indexmap::IndexMap;
use serde::Serialize;

use crate::logfile::LogFiles;
use crate::model::OutputEncoding;

pub const STREAM_OUT: u8 = 0;
pub const STREAM_ERR: u8 = 1;
/// 대시보드가 남기는 안내 줄 (실행 명령, 종료 코드 등)
pub const STREAM_SYS: u8 = 2;

pub const DEFAULT_CAPACITY: usize = 20_000;
/// 줄바꿈 없이 이 길이를 넘으면 강제로 한 줄로 끊는다.
const MAX_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub seq: u64,
    pub ts: u64,
    pub stream: u8,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogBatch {
    pub id: String,
    /// 화면의 콘솔도 비우고 이 줄들부터 다시 보여 줄 것 (새로 시작할 때 이전 실행 로그 지우기)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reset: bool,
    pub lines: Vec<LogLine>,
}

#[derive(Default)]
struct Pending {
    reset: bool,
    lines: Vec<LogLine>,
}

#[derive(Default)]
struct ServiceLog {
    lines: VecDeque<LogLine>,
    next_seq: u64,
}

#[derive(Default)]
struct HubInner {
    buffers: HashMap<String, ServiceLog>,
    pending: IndexMap<String, Pending>,
}

pub struct LogHub {
    inner: Mutex<HubInner>,
    capacity: usize,
    files: Option<std::sync::Arc<LogFiles>>,
}

pub fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl LogHub {
    pub fn new(capacity: usize) -> Self {
        LogHub { inner: Mutex::new(HubInner::default()), capacity: capacity.max(100), files: None }
    }

    /// 로그 파일 저장을 함께 한다 (설정에서 켜고 끈다).
    pub fn with_files(mut self, files: std::sync::Arc<LogFiles>) -> Self {
        self.files = Some(files);
        self
    }

    pub fn push(&self, id: &str, stream: u8, text: String) {
        let line = {
            let mut inner = self.inner.lock().unwrap();
            let buf = inner.buffers.entry(id.to_string()).or_default();
            buf.next_seq += 1;
            let line = LogLine { seq: buf.next_seq, ts: now_millis(), stream, text };
            buf.lines.push_back(line.clone());
            while buf.lines.len() > self.capacity {
                buf.lines.pop_front();
            }
            let pending = &mut inner.pending.entry(id.to_string()).or_default().lines;
            pending.push(line.clone());
            if pending.len() > self.capacity {
                let excess = pending.len() - self.capacity;
                pending.drain(..excess);
            }
            line
        };
        // 파일 쓰기는 버퍼 잠금 밖에서 한다.
        if let Some(files) = &self.files {
            files.write(id, &line);
        }
    }

    pub fn snapshot(&self, id: &str) -> Vec<LogLine> {
        let inner = self.inner.lock().unwrap();
        inner.buffers.get(id).map(|b| b.lines.iter().cloned().collect()).unwrap_or_default()
    }

    /// 버퍼를 비운다. 순번(seq)은 이어서 증가한다.
    pub fn clear(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(b) = inner.buffers.get_mut(id) {
            b.lines.clear();
        }
        inner.pending.shift_remove(id);
    }

    /// 버퍼를 비우고 화면에도 비우라고 알린다 (다음 묶음에 `reset`이 붙는다).
    /// 아직 보내지 않은 이전 줄은 버린다.
    pub fn reset(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(b) = inner.buffers.get_mut(id) {
            b.lines.clear();
        }
        let p = inner.pending.entry(id.to_string()).or_default();
        p.lines.clear();
        p.reset = true;
    }

    pub fn take_pending(&self) -> Vec<LogBatch> {
        let mut inner = self.inner.lock().unwrap();
        std::mem::take(&mut inner.pending)
            .into_iter()
            .filter(|(_, p)| p.reset || !p.lines.is_empty())
            .map(|(id, p)| LogBatch { id, reset: p.reset, lines: p.lines })
            .collect()
    }
}

pub fn decode(bytes: &[u8], encoding: OutputEncoding) -> String {
    match encoding {
        OutputEncoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        // encoding_rs의 EUC_KR은 WHATWG 정의상 CP949(windows-949)와 같다.
        OutputEncoding::Cp949 => EUC_KR.decode_without_bom_handling(bytes).0.into_owned(),
        OutputEncoding::Auto => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => EUC_KR.decode_without_bom_handling(bytes).0.into_owned(),
        },
    }
}

/// 바이트 조각을 받아 줄 단위로 내보낸다.
#[derive(Default)]
pub struct LineAssembler {
    pending: Vec<u8>,
}

impl LineAssembler {
    pub fn feed(&mut self, chunk: &[u8], mut emit: impl FnMut(&[u8])) {
        let mut rest = chunk;
        while let Some(pos) = rest.iter().position(|&b| b == b'\n') {
            self.pending.extend_from_slice(&rest[..pos]);
            emit(clean_line(&self.pending));
            self.pending.clear();
            rest = &rest[pos + 1..];
        }
        self.pending.extend_from_slice(rest);
        if self.pending.len() > MAX_LINE_BYTES {
            emit(clean_line(&self.pending));
            self.pending.clear();
        }
    }

    /// 줄바꿈 없이 남아 있는 내용을 내보낸다 (프롬프트, 진행률 표시 등).
    pub fn flush(&mut self, mut emit: impl FnMut(&[u8])) {
        if !self.pending.is_empty() {
            let line = clean_line(&self.pending);
            if !line.is_empty() {
                emit(line);
            }
            self.pending.clear();
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

/// CRLF의 CR을 떼고, 줄 중간의 CR(진행률 덮어쓰기)은 마지막 내용만 남긴다.
fn clean_line(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    match line.iter().rposition(|&b| b == b'\r') {
        Some(pos) if pos + 1 < line.len() => &line[pos + 1..],
        Some(pos) => &line[..pos],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(chunks: &[&[u8]]) -> Vec<String> {
        let mut a = LineAssembler::default();
        let mut out = Vec::new();
        for c in chunks {
            a.feed(c, |l| out.push(String::from_utf8_lossy(l).into_owned()));
        }
        a.flush(|l| out.push(String::from_utf8_lossy(l).into_owned()));
        out
    }

    #[test]
    fn splits_lines_across_chunks() {
        assert_eq!(collect(&[b"hel", b"lo\r\nwor", b"ld\n", b"tail"]), vec!["hello", "world", "tail"]);
    }

    #[test]
    fn carriage_return_keeps_last_segment() {
        assert_eq!(collect(&[b"10%\r50%\r100%\n"]), vec!["100%"]);
        assert_eq!(collect(&[b"done\r\n"]), vec!["done"]);
    }

    #[test]
    fn decodes_cp949_in_auto_mode() {
        // "한글"을 CP949로 인코딩한 바이트
        let (bytes, _, _) = EUC_KR.encode("한글 로그");
        assert_eq!(decode(&bytes, OutputEncoding::Auto), "한글 로그");
        assert_eq!(decode("한글".as_bytes(), OutputEncoding::Auto), "한글");
        assert_eq!(decode(&bytes, OutputEncoding::Cp949), "한글 로그");
    }

    #[test]
    fn ring_buffer_caps_and_keeps_seq() {
        let hub = LogHub::new(100);
        for i in 0..250 {
            hub.push("a", STREAM_OUT, format!("line {i}"));
        }
        let snap = hub.snapshot("a");
        assert_eq!(snap.len(), 100);
        assert_eq!(snap.first().unwrap().seq, 151);
        assert_eq!(snap.last().unwrap().seq, 250);
        let pending = hub.take_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].lines.len(), 100);
        assert!(hub.take_pending().is_empty());
        hub.clear("a");
        hub.push("a", STREAM_OUT, "after".into());
        assert_eq!(hub.snapshot("a")[0].seq, 251);
    }

    #[test]
    fn reset_drops_old_lines_and_tells_the_screen() {
        let hub = LogHub::new(100);
        hub.push("a", STREAM_OUT, "old 1".into());
        hub.take_pending();
        hub.push("a", STREAM_OUT, "old 2 (아직 안 보냄)".into());
        hub.push("b", STREAM_OUT, "other".into());
        hub.reset("a");
        hub.push("a", STREAM_SYS, "▶ new run".into());
        assert_eq!(hub.snapshot("a").iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["▶ new run"]);
        let batches = hub.take_pending();
        let a = batches.iter().find(|b| b.id == "a").unwrap();
        assert!(a.reset, "화면도 비우라는 표시");
        assert_eq!(a.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["▶ new run"], "보내지 않은 이전 줄은 버림");
        assert!(!batches.iter().find(|b| b.id == "b").unwrap().reset);
        // 줄이 없어도 reset만 따로 보낸다
        hub.reset("b");
        let batches = hub.take_pending();
        assert_eq!(batches.len(), 1);
        assert!(batches[0].reset && batches[0].lines.is_empty());
        assert!(hub.take_pending().is_empty());
        assert!(serde_json::to_string(&LogBatch { id: "x".into(), reset: false, lines: vec![] }).unwrap().find("reset").is_none());
    }
}
