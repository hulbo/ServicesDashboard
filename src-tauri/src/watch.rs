//! 로그와 포트를 보고 서비스가 준비됐는지, 어떤 포트·URL로 떠 있는지 알아낸다.

use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

use crate::model::{Service, ServiceKind};

static ANSI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-Z\\-_]").unwrap());

/// 로컬에서 접속할 수 있는 URL (외부 IP 주소는 제외)
static URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(https?)://(localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1?\]):(\d{2,5})(/[^\s'"<>()\]]*)?"#).unwrap()
});

/// "Tomcat started on port 8080", "Netty started on port(s): 8080", "Jetty started on port(s) 8080"
static SERVER_PORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:tomcat|netty|undertow|jetty)\b.*?\bstarted on port(?:\(s\))?:?\s*(\d{2,5})").unwrap()
});
static CONTEXT_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"context path '([^']*)'").unwrap());

/// Spring Boot 기동 완료
const SPRING_READY: &[&str] = &[r"Started \S+ in [\d.]+ seconds"];
/// 흔한 프론트엔드 개발 서버·Node 서버의 준비 메시지
const NODE_READY: &[&str] = &[
    r"Local:\s+https?://",
    r"(?i)\bready in \d",
    r"(?i)compiled successfully",
    r"(?i)webpack \S*\s*compiled",
    r"(?i)server (?:is )?(?:running|listening|started) (?:at|on)",
    r"(?i)listening on (?:port )?(?:https?://)?\S*\d{2,5}",
];

pub fn strip_ansi(s: &str) -> Cow<'_, str> {
    if s.contains('\x1b') { ANSI.replace_all(s, "") } else { Cow::Borrowed(s) }
}

/// 한 줄에서 찾은 포트와 URL
#[derive(Debug, Default, PartialEq)]
pub struct Found {
    pub ports: Vec<u16>,
    pub urls: Vec<String>,
}

pub fn detect(plain: &str) -> Found {
    let mut found = Found::default();
    let lower_hint = plain.contains("http") || plain.contains("port") || plain.contains("Port");
    if !lower_hint {
        return found;
    }
    for c in URL.captures_iter(plain) {
        let Ok(port) = c[3].parse::<u16>() else { continue };
        let path = c.get(4).map(|m| m.as_str().trim_end_matches(['.', ',', ';', ':'])).unwrap_or("/");
        let path = if path.is_empty() { "/" } else { path };
        let url = format!("{}://localhost:{port}{path}", c[1].to_lowercase());
        push_unique(&mut found.ports, port);
        if !found.urls.contains(&url) {
            found.urls.push(url);
        }
    }
    if let Some(c) = SERVER_PORT.captures(plain) {
        if let Ok(port) = c[1].parse::<u16>() {
            let ctx = CONTEXT_PATH.captures(plain).map(|m| m[1].to_string()).unwrap_or_default();
            let ctx = if ctx.is_empty() || ctx == "/" { "/".to_string() } else { format!("{}/", ctx.trim_end_matches('/')) };
            push_unique(&mut found.ports, port);
            let url = format!("http://localhost:{port}{ctx}");
            if !found.urls.contains(&url) {
                found.urls.push(url);
            }
        }
    }
    found
}

fn push_unique(v: &mut Vec<u16>, p: u16) {
    if !v.contains(&p) {
        v.push(p);
    }
}

/// 준비 판정 방법
#[derive(Debug, Clone)]
pub struct ReadySpec {
    pub patterns: Vec<Regex>,
    pub ports: Vec<u16>,
    pub http_url: Option<String>,
    pub timeout: Duration,
}

impl ReadySpec {
    pub fn matches(&self, plain: &str) -> bool {
        self.patterns.iter().any(|r| r.is_match(plain))
    }
    pub fn needs_probe(&self) -> bool {
        !self.ports.is_empty() || self.http_url.is_some()
    }
}

pub fn default_patterns(kind: ServiceKind) -> &'static [&'static str] {
    match kind {
        ServiceKind::GradleBoot | ServiceKind::MavenBoot | ServiceKind::JavaMain | ServiceKind::Jar => SPRING_READY,
        ServiceKind::Npm | ServiceKind::Node => NODE_READY,
        _ => &[],
    }
}

/// 서비스의 준비 판정 방법. None이면 판정 없이 바로 '실행 중'.
/// `http_url`은 변수 치환을 마친 값을 넘긴다.
pub fn ready_spec(svc: &Service, http_url: Option<String>) -> Result<Option<ReadySpec>, String> {
    let opts = svc.readiness.clone().unwrap_or_default();
    if opts.disabled == Some(true) {
        return Ok(None);
    }
    let patterns: Vec<Regex> = match opts.log_pattern.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => vec![Regex::new(p).map_err(|e| format!("준비 감지 정규식이 올바르지 않습니다: {e}"))?],
        None => default_patterns(svc.kind).iter().map(|p| Regex::new(p).expect("기본 패턴")).collect(),
    };
    let ports = svc.ports.clone();
    if patterns.is_empty() && ports.is_empty() && http_url.is_none() {
        return Ok(None);
    }
    let default_timeout = match svc.kind {
        ServiceKind::GradleBoot | ServiceKind::MavenBoot | ServiceKind::JavaMain => 300,
        _ => 120,
    };
    Ok(Some(ReadySpec {
        patterns,
        ports,
        http_url,
        timeout: Duration::from_secs(opts.timeout_secs.unwrap_or(default_timeout).clamp(1, 3600)),
    }))
}

/// 로컬 포트가 연결을 받는지 (IPv4·IPv6)
pub fn port_open(port: u16) -> bool {
    ["127.0.0.1", "[::1]"].iter().any(|host| {
        format!("{host}:{port}")
            .parse()
            .is_ok_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok())
    })
}

/// HTTP GET이 2xx·3xx로 응답하는지 (https는 포트 연결만 확인)
pub fn http_ok(url: &str) -> bool {
    let (scheme, rest) = match url.split_once("://") {
        Some(x) => x,
        None => ("http", url),
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let default_port = if scheme.eq_ignore_ascii_case("https") { 443 } else { 80 };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h, p.parse().unwrap()),
        _ => (hostport, default_port),
    };
    if scheme.eq_ignore_ascii_case("https") {
        return port_open(port);
    }
    let Ok(addrs) = (host.trim_matches(['[', ']']), port).to_socket_addrs() else { return false };
    for addr in addrs {
        let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else { continue };
        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
        let req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: ServicesDashboard\r\nConnection: close\r\n\r\n");
        if s.write_all(req.as_bytes()).is_err() {
            continue;
        }
        let mut buf = [0u8; 32];
        let n = s.read(&mut buf).unwrap_or(0);
        let head = String::from_utf8_lossy(&buf[..n]);
        if let Some(code) = head.split_whitespace().nth(1).and_then(|c| c.parse::<u16>().ok()) {
            return (200..400).contains(&code);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ReadinessOpts;

    #[test]
    fn strips_ansi() {
        assert_eq!(strip_ansi("\x1b[32m\x1b[1mVITE\x1b[22m v5\x1b[39m"), "VITE v5");
        assert_eq!(strip_ansi("\x1b]0;title\x07ok"), "ok");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn detects_urls_and_server_ports() {
        let f = detect("  ➜  Local:   http://localhost:5173/");
        assert_eq!(f, Found { ports: vec![5173], urls: vec!["http://localhost:5173/".into()] });
        let f = detect("  ➜  Network: http://192.168.0.10:5173/");
        assert_eq!(f, Found::default(), "외부 IP는 무시");
        let f = detect("Server running at http://0.0.0.0:3000.");
        assert_eq!(f.urls, vec!["http://localhost:3000/"]);
        let f = detect("o.s.b.w.embedded.tomcat.TomcatWebServer  : Tomcat started on port 8080 (http) with context path '/api'");
        assert_eq!(f, Found { ports: vec![8080], urls: vec!["http://localhost:8080/api/".into()] });
        let f = detect("Tomcat started on port(s): 8081 (http) with context path ''");
        assert_eq!(f.urls, vec!["http://localhost:8081/"]);
        assert_eq!(detect("Netty started on port 9090").ports, vec![9090]);
        assert_eq!(detect("Jetty started on port(s) 8082 (http/1.1)").ports, vec![8082]);
        assert_eq!(detect("nothing here"), Found::default());
    }

    #[test]
    fn default_ready_patterns() {
        let svc = Service { kind: ServiceKind::GradleBoot, ..Default::default() };
        let spec = ready_spec(&svc, None).unwrap().unwrap();
        assert!(spec.matches("k.e.PortalApplication : Started PortalApplication in 3.412 seconds (process running for 3.9)"));
        assert!(!spec.matches("Starting PortalApplication using Java 17"));
        assert_eq!(spec.timeout, Duration::from_secs(300));

        let svc = Service { kind: ServiceKind::Npm, ..Default::default() };
        let spec = ready_spec(&svc, None).unwrap().unwrap();
        for line in ["  ➜  Local:   http://localhost:5173/", "VITE v5.4.10  ready in 812 ms", "webpack 5.90.1 compiled successfully in 1234 ms", "Server listening on port 3000"] {
            assert!(spec.matches(line), "{line}");
        }
        // 셸 명령은 기본 판정이 없어 바로 '실행 중'
        let svc = Service { kind: ServiceKind::Shell, ..Default::default() };
        assert!(ready_spec(&svc, None).unwrap().is_none());
        // 포트를 지정하면 포트 확인으로 판정
        let svc = Service { kind: ServiceKind::Shell, ports: vec![6379], ..Default::default() };
        assert!(ready_spec(&svc, None).unwrap().unwrap().needs_probe());
    }

    #[test]
    fn custom_and_disabled_readiness() {
        let mut svc = Service { kind: ServiceKind::Npm, ..Default::default() };
        svc.readiness = Some(ReadinessOpts { log_pattern: Some("^READY$".into()), timeout_secs: Some(5), ..Default::default() });
        let spec = ready_spec(&svc, None).unwrap().unwrap();
        assert!(spec.matches("READY") && !spec.matches("Local: http://localhost:5173"));
        assert_eq!(spec.timeout, Duration::from_secs(5));
        svc.readiness = Some(ReadinessOpts { log_pattern: Some("([".into()), ..Default::default() });
        assert!(ready_spec(&svc, None).unwrap_err().contains("정규식"));
        svc.readiness = Some(ReadinessOpts { disabled: Some(true), ..Default::default() });
        assert!(ready_spec(&svc, None).unwrap().is_none());
    }

    #[test]
    fn probes_port_and_http() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(port_open(port));
        std::thread::spawn(move || {
            // port_open의 연결 1개 + HTTP 요청 2개
            for stream in listener.incoming().take(3) {
                let mut s = stream.unwrap();
                let mut buf = [0u8; 512];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let status = if req.starts_with("GET /health") { "200 OK" } else { "503 Service Unavailable" };
                let _ = s.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n").as_bytes());
            }
        });
        assert!(http_ok(&format!("http://localhost:{port}/health")));
        assert!(!http_ok(&format!("http://127.0.0.1:{port}/down")));
        let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(!port_open(free));
    }
}
