//! JDK / Node / Maven 런타임 감지와 별칭 해석.
//!
//! 서비스 설정의 `runtime.jdk`에는 절대 경로 대신 `17`, `1.8` 같은 별칭을 쓸 수 있다.
//! 별칭은 감지된 런타임 중 주 버전이 같은 것(여러 개면 가장 높은 버전)으로 바뀐다.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeKind {
    Jdk,
    Node,
    Maven,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub kind: RuntimeKind,
    /// JDK·Maven: 홈 디렉터리, Node: node.exe가 있는 디렉터리 (슬래시 구분)
    pub path: String,
    pub version: String,
    /// 별칭 비교용 주 버전 (예: 1.8.0_202 → "8")
    pub major: String,
    pub vendor: Option<String>,
    /// 어디서 찾았는지 (JAVA_HOME, PATH, IntelliJ 등)
    pub source: String,
}

/// 감지에 쓰는 환경. 테스트에서 가짜 디렉터리를 넣을 수 있게 분리했다.
#[derive(Debug, Clone, Default)]
pub struct DetectEnv {
    pub java_home: Option<PathBuf>,
    pub maven_homes: Vec<PathBuf>,
    pub path_dirs: Vec<PathBuf>,
    pub program_files: Vec<PathBuf>,
    pub user_home: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub local_appdata: Option<PathBuf>,
    pub nvm_home: Option<PathBuf>,
    /// IntelliJ 설정 폴더들 (`%APPDATA%\JetBrains\IntelliJIdea2024.1` 등, 최신 순)
    pub jetbrains_dirs: Vec<PathBuf>,
    /// 버전 파일이 없을 때 실행 파일을 돌려 버전을 알아낼지
    pub probe_executables: bool,
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

impl DetectEnv {
    pub fn from_system() -> Self {
        let mut program_files = Vec::new();
        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
            if let Some(p) = env_path(var) {
                if !program_files.contains(&p) {
                    program_files.push(p);
                }
            }
        }
        let appdata = env_path("APPDATA");
        DetectEnv {
            java_home: env_path("JAVA_HOME"),
            maven_homes: ["MAVEN_HOME", "M2_HOME"].iter().filter_map(|v| env_path(v)).collect(),
            path_dirs: std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default(),
            program_files,
            user_home: dirs::home_dir(),
            local_appdata: env_path("LOCALAPPDATA"),
            nvm_home: env_path("NVM_HOME"),
            jetbrains_dirs: jetbrains_config_dirs(appdata.as_deref()),
            appdata,
            probe_executables: true,
        }
    }
}

/// `%APPDATA%\JetBrains` 아래 IntelliJ 설정 폴더 (최신 버전 먼저)
pub fn jetbrains_config_dirs(appdata: Option<&Path>) -> Vec<PathBuf> {
    let Some(base) = appdata.map(|a| a.join("JetBrains")) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = list_dirs(&base)
        .into_iter()
        .filter(|d| {
            let n = file_name(d);
            n.starts_with("IntelliJIdea") || n.starts_with("IdeaIC")
        })
        .collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(file_name(d)));
    dirs
}

/// IntelliJ `jdk.table.xml`의 (이름, 홈 경로) 목록
pub fn intellij_jdks(jetbrains_dirs: &[PathBuf], user_home: Option<&Path>) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for dir in jetbrains_dirs {
        let Ok(text) = std::fs::read_to_string(dir.join("options").join("jdk.table.xml")) else { continue };
        let Ok(doc) = roxmltree::Document::parse(&text) else { continue };
        for jdk in doc.descendants().filter(|n| n.has_tag_name("jdk")) {
            let value = |tag: &str| {
                jdk.children().find(|c| c.has_tag_name(tag)).and_then(|c| c.attribute("value")).map(str::to_string)
            };
            if value("type").as_deref().is_some_and(|t| t != "JavaSDK") {
                continue;
            }
            if let (Some(name), Some(home)) = (value("name"), value("homePath")) {
                let home = match user_home {
                    Some(h) => home.replace("$USER_HOME$", &h.to_string_lossy()),
                    None => home,
                };
                out.push((name, PathBuf::from(home)));
            }
        }
    }
    out
}

pub fn detect(env: &DetectEnv) -> Vec<RuntimeInfo> {
    let mut out = Vec::new();
    out.extend(detect_jdks(env));
    out.extend(detect_nodes(env));
    out.extend(detect_mavens(env));
    out
}

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn list_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

pub fn to_slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn dedupe_key(p: &Path) -> String {
    let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    to_slash(&p).trim_start_matches("//?/").trim_end_matches('/').to_lowercase()
}

// ---------------------------------------------------------------- JDK

fn detect_jdks(env: &DetectEnv) -> Vec<RuntimeInfo> {
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();
    if let Some(h) = &env.java_home {
        candidates.push((h.clone(), "JAVA_HOME".into()));
    }
    for pf in &env.program_files {
        for vendor in [
            "Java",
            "Eclipse Adoptium",
            "Eclipse Foundation",
            "AdoptOpenJDK",
            "Zulu",
            "Amazon Corretto",
            "Microsoft",
            "BellSoft",
            "Semeru",
            "OpenJDK",
            "RedHat",
            "Liberica",
        ] {
            for d in list_dirs(&pf.join(vendor)) {
                candidates.push((d, "설치됨".into()));
            }
        }
    }
    if let Some(home) = &env.user_home {
        for d in list_dirs(&home.join(".jdks")) {
            candidates.push((d, "IntelliJ 다운로드".into()));
        }
    }
    for (_, home) in intellij_jdks(&env.jetbrains_dirs, env.user_home.as_deref()) {
        candidates.push((home, "IntelliJ".into()));
    }
    for dir in &env.path_dirs {
        if dir.join(exe("java")).is_file() && file_name(dir).eq_ignore_ascii_case("bin") {
            if let Some(home) = dir.parent() {
                candidates.push((home.to_path_buf(), "PATH".into()));
            }
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (dir, source) in candidates {
        if !dir.join("bin").join(exe("java")).is_file() || !seen.insert(dedupe_key(&dir)) {
            continue;
        }
        let (version, vendor) = jdk_version(&dir, env.probe_executables);
        let Some(version) = version else { continue };
        out.push(RuntimeInfo {
            kind: RuntimeKind::Jdk,
            path: to_slash(&dir),
            major: java_major(&version),
            version,
            vendor,
            source,
        });
    }
    out
}

/// JDK 버전: `release` 파일 → 폴더 이름 → `java -version` 순서로 알아낸다.
fn jdk_version(dir: &Path, probe: bool) -> (Option<String>, Option<String>) {
    if let Ok(text) = std::fs::read_to_string(dir.join("release")) {
        let field = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(&format!("{key}=")))
                .map(|v| v.trim().trim_matches('"').to_string())
                .filter(|v| !v.is_empty())
        };
        if let Some(v) = field("JAVA_VERSION") {
            return (Some(v), field("IMPLEMENTOR"));
        }
    }
    if let Some(v) = version_from_name(&file_name(dir)) {
        return (Some(v), None);
    }
    if probe {
        if let Some(out) = run_quick(&dir.join("bin").join(exe("java")), &["-version"]) {
            if let Some(v) = out.split('"').nth(1) {
                return (Some(v.to_string()), None);
            }
        }
    }
    (None, None)
}

fn version_from_name(name: &str) -> Option<String> {
    // jdk1.8.0_202, jdk-17.0.15, zulu-21, corretto-17.0.9, temurin-21.0.2+13
    let start = name.find(|c: char| c.is_ascii_digit())?;
    let v: String = name[start..].chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '_')).collect();
    let v = v.trim_end_matches(['.', '_']).to_string();
    (!v.is_empty()).then_some(v)
}

/// JDK 홈의 주 버전 (release 파일 기준)
pub fn jdk_major_at(home: &Path) -> Option<u32> {
    let (v, _) = jdk_version(home, false);
    v.and_then(|v| java_major(&v).parse().ok())
}

pub fn java_major(version: &str) -> String {
    let mut parts = version.split(['.', '_', '-', '+']);
    match (parts.next(), parts.next()) {
        (Some("1"), Some(minor)) => minor.to_string(),
        (Some(major), _) => major.to_string(),
        _ => version.to_string(),
    }
}

// ---------------------------------------------------------------- Node

fn detect_nodes(env: &DetectEnv) -> Vec<RuntimeInfo> {
    let mut candidates: Vec<(PathBuf, String)> = Vec::new();
    for dir in &env.path_dirs {
        candidates.push((dir.clone(), "PATH".into()));
    }
    for pf in &env.program_files {
        candidates.push((pf.join("nodejs"), "설치됨".into()));
    }
    let nvm = env.nvm_home.clone().or_else(|| env.appdata.as_ref().map(|a| a.join("nvm")));
    if let Some(nvm) = nvm {
        for d in list_dirs(&nvm) {
            candidates.push((d, "nvm".into()));
        }
    }
    if let Some(la) = &env.local_appdata {
        for d in list_dirs(&la.join("Volta").join("tools").join("image").join("node")) {
            candidates.push((d, "Volta".into()));
        }
    }
    if let Some(a) = &env.appdata {
        for d in list_dirs(&a.join("fnm").join("node-versions")) {
            candidates.push((d.join("installation"), "fnm".into()));
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (dir, source) in candidates {
        if !dir.join(exe("node")).is_file() || !seen.insert(dedupe_key(&dir)) {
            continue;
        }
        let from_name = {
            let n = file_name(&dir);
            let n = n.strip_prefix('v').unwrap_or(&n).to_string();
            (n.split('.').count() == 3 && n.split('.').all(|p| p.parse::<u32>().is_ok())).then_some(n)
        };
        let version = from_name.or_else(|| {
            env.probe_executables
                .then(|| run_quick(&dir.join(exe("node")), &["--version"]))
                .flatten()
                .map(|v| v.trim().trim_start_matches('v').to_string())
        });
        let Some(version) = version.filter(|v| !v.is_empty()) else { continue };
        out.push(RuntimeInfo {
            kind: RuntimeKind::Node,
            path: to_slash(&dir),
            major: version.split('.').next().unwrap_or("").to_string(),
            version,
            vendor: None,
            source,
        });
    }
    out
}

// ---------------------------------------------------------------- Maven

fn detect_mavens(env: &DetectEnv) -> Vec<RuntimeInfo> {
    let mut candidates: Vec<(PathBuf, String)> = env.maven_homes.iter().map(|h| (h.clone(), "MAVEN_HOME".into())).collect();
    let mvn = if cfg!(windows) { "mvn.cmd" } else { "mvn" };
    for dir in &env.path_dirs {
        if dir.join(mvn).is_file() {
            if let Some(home) = dir.parent() {
                candidates.push((home.to_path_buf(), "PATH".into()));
            }
        }
    }
    for pf in &env.program_files {
        for ide in list_dirs(&pf.join("JetBrains")) {
            candidates.push((ide.join("plugins").join("maven").join("lib").join("maven3"), "IntelliJ 번들".into()));
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (home, source) in candidates {
        if !home.join("bin").join(mvn).is_file() || !seen.insert(dedupe_key(&home)) {
            continue;
        }
        // lib/maven-core-3.9.11.jar 파일 이름에서 버전을 읽는다
        let version = std::fs::read_dir(home.join("lib")).ok().and_then(|rd| {
            rd.flatten().find_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.strip_prefix("maven-core-").and_then(|r| r.strip_suffix(".jar")).map(str::to_string)
            })
        });
        let version = version.unwrap_or_else(|| "알 수 없음".into());
        out.push(RuntimeInfo {
            kind: RuntimeKind::Maven,
            path: to_slash(&home),
            major: version.split('.').next().unwrap_or("").to_string(),
            version,
            vendor: None,
            source,
        });
    }
    out
}

/// 버전 확인용으로 실행 파일을 짧게 돌린다 (창을 띄우지 않고, 3초 제한).
fn run_quick(program: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().ok()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Some(text)
}

// ---------------------------------------------------------------- 별칭 해석

fn version_key(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect()
}

/// `17`, `jdk17`, `1.8`, `node20`, `v20.11` 같은 값을 주 버전으로 정규화한다.
fn normalize_alias(kind: RuntimeKind, value: &str) -> String {
    let v = value.trim().to_lowercase();
    let v = ["jdk-", "jdk", "java-", "java", "node-", "node", "v"]
        .iter()
        .find_map(|p| v.strip_prefix(p))
        .unwrap_or(&v)
        .to_string();
    if kind == RuntimeKind::Jdk { java_major(&v) } else { v.split('.').next().unwrap_or(&v).to_string() }
}

/// 경로면 그대로, 별칭이면 감지된 런타임 중 가장 높은 버전의 경로를 돌려준다.
pub fn resolve(kind: RuntimeKind, value: &str, runtimes: &[RuntimeInfo]) -> Result<String, String> {
    let value = value.trim();
    if Path::new(value).is_dir() {
        return Ok(value.to_string());
    }
    let label = match kind {
        RuntimeKind::Jdk => "JDK",
        RuntimeKind::Node => "Node",
        RuntimeKind::Maven => "Maven",
    };
    let looks_like_alias = value.len() <= 16 && !value.contains(['/', '\\', ':']);
    if !looks_like_alias {
        return Err(format!("{label} 경로가 없습니다: {value}"));
    }
    let want = normalize_alias(kind, value);
    runtimes
        .iter()
        .filter(|r| r.kind == kind && (r.major == want || r.version == value.trim_start_matches('v')))
        .max_by(|a, b| version_key(&a.version).cmp(&version_key(&b.version)))
        .map(|r| r.path.clone())
        .ok_or_else(|| format!("{label} '{value}'에 맞는 설치를 찾지 못했습니다. 런타임 목록에서 확인하거나 경로를 직접 입력하세요."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "").unwrap();
    }

    fn fake_env(root: &Path) -> DetectEnv {
        DetectEnv {
            java_home: Some(root.join("pf/Java/jdk-17.0.15")),
            program_files: vec![root.join("pf")],
            user_home: Some(root.join("home")),
            appdata: Some(root.join("appdata")),
            path_dirs: vec![root.join("pf/nodejs"), root.join("legacy/jre/bin")],
            jetbrains_dirs: vec![root.join("appdata/JetBrains/IntelliJIdea2024.1")],
            probe_executables: false,
            ..Default::default()
        }
    }

    fn build_fixture(root: &Path) {
        let jdk17 = root.join("pf/Java/jdk-17.0.15");
        touch(&jdk17.join("bin").join(exe("java")));
        fs::write(jdk17.join("release"), "JAVA_VERSION=\"17.0.15\"\nIMPLEMENTOR=\"Oracle Corporation\"\n").unwrap();
        let jdk21 = root.join("pf/Eclipse Adoptium/jdk-21.0.2.13-hotspot");
        touch(&jdk21.join("bin").join(exe("java")));
        fs::write(jdk21.join("release"), "JAVA_VERSION=\"21.0.2\"\nIMPLEMENTOR=\"Eclipse Adoptium\"\n").unwrap();
        touch(&root.join("pf/Java/jdk1.8.0_202/bin").join(exe("java")));
        touch(&root.join("home/.jdks/corretto-17.0.9/bin").join(exe("java")));
        touch(&root.join("legacy/jre/bin").join(exe("java")));
        let ij = root.join("ij-jdk-24");
        touch(&ij.join("bin").join(exe("java")));
        fs::write(ij.join("release"), "JAVA_VERSION=\"24\"\n").unwrap();
        let table = root.join("appdata/JetBrains/IntelliJIdea2024.1/options/jdk.table.xml");
        fs::create_dir_all(table.parent().unwrap()).unwrap();
        fs::write(
            &table,
            format!(
                r#"<application><component name="ProjectJdkTable"><jdk version="2"><name value="24" /><type value="JavaSDK" /><homePath value="{}" /></jdk><jdk version="2"><name value="Python" /><type value="Python SDK" /><homePath value="C:/py" /></jdk></component></application>"#,
                to_slash(&ij)
            ),
        )
        .unwrap();
        touch(&root.join("pf/nodejs").join(exe("node")));
        touch(&root.join("appdata/nvm/v18.20.4").join(exe("node")));
        touch(&root.join("appdata/nvm/v20.11.1").join(exe("node")));
        let mvn = root.join("pf/JetBrains/IntelliJ IDEA 2024.1/plugins/maven/lib/maven3");
        touch(&mvn.join("bin").join(if cfg!(windows) { "mvn.cmd" } else { "mvn" }));
        touch(&mvn.join("lib/maven-core-3.9.6.jar"));
    }

    #[test]
    fn detects_jdks_nodes_and_maven() {
        let dir = tempfile::tempdir().unwrap();
        build_fixture(dir.path());
        let found = detect(&fake_env(dir.path()));
        let jdks: Vec<_> = found.iter().filter(|r| r.kind == RuntimeKind::Jdk).collect();
        let by_version = |v: &str| jdks.iter().find(|r| r.version == v);
        assert_eq!(by_version("17.0.15").unwrap().source, "JAVA_HOME");
        assert_eq!(by_version("17.0.15").unwrap().vendor.as_deref(), Some("Oracle Corporation"));
        assert_eq!(by_version("21.0.2").unwrap().major, "21");
        assert_eq!(by_version("1.8.0_202").unwrap().major, "8");
        assert_eq!(by_version("17.0.9").unwrap().source, "IntelliJ 다운로드");
        assert_eq!(by_version("24").unwrap().source, "IntelliJ");
        // JAVA_HOME과 Program Files에서 같은 JDK가 두 번 나오지 않는다
        assert_eq!(jdks.iter().filter(|r| r.version == "17.0.15").count(), 1);
        // 버전 파일도, 폴더 이름의 버전도 없고 실행해 보지도 않으면 목록에서 뺀다
        assert!(!jdks.iter().any(|r| r.path.contains("legacy/jre")));

        let nodes: Vec<_> = found.iter().filter(|r| r.kind == RuntimeKind::Node).map(|r| r.version.as_str()).collect();
        assert!(nodes.contains(&"18.20.4") && nodes.contains(&"20.11.1"));
        let mvn = found.iter().find(|r| r.kind == RuntimeKind::Maven).unwrap();
        assert_eq!(mvn.version, "3.9.6");
        assert_eq!(mvn.source, "IntelliJ 번들");
    }

    #[test]
    fn resolves_aliases() {
        let dir = tempfile::tempdir().unwrap();
        build_fixture(dir.path());
        let found = detect(&fake_env(dir.path()));
        let jdk17 = resolve(RuntimeKind::Jdk, "17", &found).unwrap();
        assert!(jdk17.ends_with("jdk-17.0.15"), "가장 높은 17.x를 고른다: {jdk17}");
        assert!(resolve(RuntimeKind::Jdk, "1.8", &found).unwrap().ends_with("jdk1.8.0_202"));
        assert!(resolve(RuntimeKind::Jdk, "jdk21", &found).unwrap().contains("jdk-21"));
        assert!(resolve(RuntimeKind::Node, "node20", &found).unwrap().ends_with("v20.11.1"));
        assert!(resolve(RuntimeKind::Jdk, "11", &found).unwrap_err().contains("찾지 못했습니다"));
        assert!(resolve(RuntimeKind::Jdk, "C:/no/such/jdk", &found).unwrap_err().contains("경로가 없습니다"));
        let real = to_slash(&dir.path().join("pf/Java/jdk1.8.0_202"));
        assert_eq!(resolve(RuntimeKind::Jdk, &real, &found).unwrap(), real);
    }

    #[test]
    fn version_helpers() {
        assert_eq!(java_major("1.8.0_202"), "8");
        assert_eq!(java_major("17.0.15"), "17");
        assert_eq!(java_major("21"), "21");
        assert_eq!(version_from_name("jdk1.8.0_202").as_deref(), Some("1.8.0_202"));
        assert_eq!(version_from_name("temurin-21.0.2+13").as_deref(), Some("21.0.2"));
        assert_eq!(version_from_name("jre"), None);
    }
}
