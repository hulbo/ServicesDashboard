//! 워크스페이스 파일(JSON) 모델.
//!
//! 알 수 없는 필드는 `extra`에 보관했다가 저장할 때 그대로 돌려놓는다.
//! 상위 버전에서 추가된 필드나 사용자가 직접 넣은 필드를 잃지 않기 위해서다.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub type EnvMap = IndexMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default)]
    pub name: String,
    /// `${var:NAME}`으로 참조하는 워크스페이스 변수
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub variables: EnvMap,
    /// 모든 서비스에 공통으로 적용되는 환경변수
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub env: EnvMap,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub services: Vec<Service>,
    /// 실행 프리셋: 그룹과 상관없이 고른 서비스 묶음
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<Preset>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub env: EnvMap,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceKind {
    #[default]
    Npm,
    Node,
    GradleBoot,
    MavenBoot,
    JavaMain,
    Jar,
    Gradle,
    Maven,
    Shell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutputEncoding {
    /// 줄마다 UTF-8로 해석해 보고, 실패하면 CP949로 해석한다.
    #[default]
    Auto,
    #[serde(rename = "utf-8")]
    Utf8,
    Cp949,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(rename = "type")]
    pub kind: ServiceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub env: EnvMap,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jvm_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// 실행할 메인 클래스 (gradle-boot는 init script, maven-boot는 -Dspring-boot.run.main-class로 전달)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeOpts>,
    /// 한글 깨짐 방지·컬러 로그 옵션 자동 추가 여부 (기본: 모두 켬)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inject: Option<InjectOpts>,
    /// 준비(Ready) 감지 방법. 없으면 타입별 기본값을 쓴다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<ReadinessOpts>,
    /// 먼저 준비돼야 하는 서비스 ID
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// 비정상 종료 시 자동 재시작 (있으면 켜짐)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_restart: Option<AutoRestartOpts>,
    /// '디버그로 시작'할 때 쓰는 설정
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<DebugOpts>,
    /// java-main: 클래스패스를 어디서 얻을지
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_main: Option<JavaMainOpts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spring: Option<SpringOpts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm: Option<NpmOpts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeOpts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradle: Option<GradleOpts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maven: Option<MavenOpts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jar: Option<JarOpts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<ShellOpts>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<u16>,
    #[serde(default, skip_serializing_if = "is_default_encoding")]
    pub encoding: OutputEncoding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn is_default_encoding(e: &OutputEncoding) -> bool {
    *e == OutputEncoding::Auto
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOpts {
    /// JDK 홈 디렉터리 또는 별칭 (예: `17`, `1.8`). JAVA_HOME으로 쓰인다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jdk: Option<String>,
    /// node.exe가 들어 있는 디렉터리 또는 별칭 (예: `20`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Maven 홈 디렉터리 또는 별칭. Maven Wrapper(mvnw)보다 우선한다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maven: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InjectOpts {
    /// Java: -Dfile.encoding=UTF-8 등 콘솔 인코딩 옵션 추가
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utf8: Option<bool>,
    /// Node: FORCE_COLOR=1, Spring Boot: --spring.output.ansi.enabled=always, Maven: -Dstyle.color=always
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutoRestartOpts {
    /// 연속 재시작 최대 횟수 (기본 3). 1분 넘게 실행되면 횟수를 다시 센다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    /// 첫 재시작까지 기다리는 시간(초, 기본 3). 재시도마다 두 배 (최대 60초)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_secs: Option<u64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl AutoRestartOpts {
    pub fn max(&self) -> u32 {
        self.max_retries.unwrap_or(3)
    }
    /// n번째(0부터) 재시작 전 대기 시간
    pub fn delay(&self, attempt: u32) -> std::time::Duration {
        let base = self.delay_secs.unwrap_or(3).max(1);
        std::time::Duration::from_secs(base.saturating_mul(1u64 << attempt.min(6)).min(60))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DebugOpts {
    /// 디버거 포트 (기본: Java 5005, Node 9229)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Java: 디버거가 붙을 때까지 기다린다 (suspend=y)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspend: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClasspathSource {
    /// Gradle로 컴파일하고 runtimeClasspath를 얻는다
    #[default]
    Gradle,
    /// Maven으로 컴파일하고 dependency:build-classpath로 얻는다
    Maven,
    /// 직접 적은 클래스패스만 쓴다 (빌드 안 함)
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RebuildPolicy {
    /// 소스·빌드 파일이 바뀌었을 때만 빌드
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JavaMainOpts {
    #[serde(default)]
    pub source: ClasspathSource,
    /// gradle: 프로젝트 경로 (예: :apps:api), maven: 모듈 경로 (예: api)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// manual: 클래스패스 항목 (폴더·jar)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub rebuild: RebuildPolicy,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessOpts {
    /// 이 정규식과 맞는 로그 줄이 나오면 준비 완료
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_pattern: Option<String>,
    /// 2xx/3xx로 응답하면 준비 완료 (예: http://localhost:8080/actuator/health)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_url: Option<String>,
    /// 이 시간 안에 준비 신호가 없으면 '실행 중'으로 바꾼다 (초)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// true면 준비 감지를 하지 않고 바로 '실행 중'으로 표시
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Service {
    pub fn inject_utf8(&self) -> bool {
        self.inject.as_ref().and_then(|i| i.utf8).unwrap_or(true)
    }
    pub fn inject_color(&self) -> bool {
        self.inject.as_ref().and_then(|i| i.color).unwrap_or(true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpringOpts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    #[default]
    Npm,
    Yarn,
    Pnpm,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NpmOpts {
    #[serde(default)]
    pub script: String,
    #[serde(default)]
    pub package_manager: PackageManager,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeOpts {
    #[serde(default)]
    pub entry: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_args: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GradleOpts {
    /// Gradle 프로젝트 경로 (예: `:apps:portal`). 비우면 루트 프로젝트.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// `gradle` 타입에서 실행할 태스크
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<String>,
    /// 추가 Gradle 옵션 (예: `--offline`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// Gradle 데몬 재사용 (기본: 끔 → `--no-daemon`).
    /// 켜면 빠르지만, 다른 곳(IntelliJ 등)에서 띄운 데몬에 붙으면 중지 시 앱 종료가 늦을 수 있다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MavenOpts {
    /// `-pl`에 넘길 모듈
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// `maven` 타입에서 실행할 골
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JarOpts {
    #[serde(default)]
    pub path: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ShellKind {
    #[default]
    Cmd,
    Powershell,
    Pwsh,
    Bash,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShellOpts {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub shell: ShellKind,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Workspace {
    pub fn empty(name: &str) -> Self {
        Workspace {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn service(&self, id: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.id == id)
    }

    pub fn group(&self, id: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// 저장 전에 구조적 오류를 확인한다.
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for g in &self.groups {
            if g.id.trim().is_empty() {
                return Err("그룹 ID가 비어 있습니다.".into());
            }
            if !ids.insert(format!("g:{}", g.id)) {
                return Err(format!("그룹 ID '{}'가 중복됩니다.", g.id));
            }
        }
        for s in &self.services {
            if s.id.trim().is_empty() {
                return Err("서비스 ID가 비어 있습니다.".into());
            }
            if !ids.insert(format!("s:{}", s.id)) {
                return Err(format!("서비스 ID '{}'가 중복됩니다.", s.id));
            }
            if let Some(g) = &s.group {
                if self.group(g).is_none() {
                    return Err(format!("서비스 '{}'의 그룹 '{}'이 없습니다.", s.name, g));
                }
            }
        }
        for s in &self.services {
            for d in &s.depends_on {
                if d == &s.id {
                    return Err(format!("서비스 '{}'이(가) 자기 자신에 의존합니다.", s.name));
                }
                if self.service(d).is_none() {
                    return Err(format!("서비스 '{}'의 의존 대상 '{d}'이(가) 없습니다.", s.name));
                }
            }
        }
        if let Some(cycle) = self.dependency_cycle() {
            return Err(format!("의존 관계가 순환합니다: {}", cycle.join(" → ")));
        }
        let mut preset_ids = std::collections::HashSet::new();
        for p in &self.presets {
            if p.id.trim().is_empty() || !preset_ids.insert(p.id.clone()) {
                return Err(format!("프리셋 ID '{}'가 비었거나 중복됩니다.", p.id));
            }
            if let Some(missing) = p.services.iter().find(|id| self.service(id).is_none()) {
                return Err(format!("프리셋 '{}'의 서비스 '{missing}'이(가) 없습니다.", p.name));
            }
        }
        Ok(())
    }

    /// 순환 의존이 있으면 그 경로(서비스 이름)를 돌려준다.
    pub fn dependency_cycle(&self) -> Option<Vec<String>> {
        fn visit<'a>(
            ws: &'a Workspace,
            id: &'a str,
            state: &mut std::collections::HashMap<&'a str, u8>,
            stack: &mut Vec<&'a str>,
        ) -> Option<Vec<String>> {
            match state.get(id) {
                Some(2) => return None,
                Some(1) => {
                    let start = stack.iter().position(|s| *s == id).unwrap_or(0);
                    let mut path: Vec<String> = stack[start..]
                        .iter()
                        .map(|sid| ws.service(sid).map(|s| s.name.clone()).unwrap_or_else(|| sid.to_string()))
                        .collect();
                    path.push(ws.service(id).map(|s| s.name.clone()).unwrap_or_else(|| id.to_string()));
                    return Some(path);
                }
                _ => {}
            }
            state.insert(id, 1);
            stack.push(id);
            if let Some(svc) = ws.service(id) {
                for d in &svc.depends_on {
                    if let Some(c) = visit(ws, d, state, stack) {
                        return Some(c);
                    }
                }
            }
            stack.pop();
            state.insert(id, 2);
            None
        }
        let mut state = std::collections::HashMap::new();
        for s in &self.services {
            if let Some(c) = visit(self, &s.id, &mut state, &mut Vec::new()) {
                return Some(c);
            }
        }
        None
    }

    /// ids와 그 의존 대상 전체를 의존 순서(먼저 시작할 것부터)로 돌려준다.
    pub fn launch_order(&self, ids: &[String]) -> Vec<String> {
        fn add(ws: &Workspace, id: &str, out: &mut Vec<String>, seen: &mut std::collections::HashSet<String>) {
            if !seen.insert(id.to_string()) {
                return;
            }
            if let Some(svc) = ws.service(id) {
                for d in &svc.depends_on {
                    add(ws, d, out, seen);
                }
                out.push(id.to_string());
            }
        }
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            add(self, id, &mut out, &mut seen);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_unknown_fields() {
        let json = r#"{
            "name": "ws",
            "futureField": {"a": 1},
            "groups": [{"id": "g1", "name": "server", "color": "red"}],
            "services": [{
                "id": "s1", "name": "api", "group": "g1", "type": "gradle-boot",
                "gradle": {"project": ":apps:api", "daemon": false},
                "readiness": {"logPattern": "Started"}
            }]
        }"#;
        let ws: Workspace = serde_json::from_str(json).unwrap();
        assert_eq!(ws.services[0].kind, ServiceKind::GradleBoot);
        let out = serde_json::to_value(&ws).unwrap();
        assert_eq!(out["futureField"]["a"], 1);
        assert_eq!(out["groups"][0]["color"], "red");
        assert_eq!(out["services"][0]["readiness"]["logPattern"], "Started");
        assert_eq!(out["services"][0]["gradle"]["daemon"], false);
        // 기본값은 저장하지 않는다
        assert!(out["services"][0].get("encoding").is_none());
    }

    fn svc(id: &str, deps: &[&str]) -> Service {
        Service { id: id.into(), name: id.to_uppercase(), depends_on: deps.iter().map(|d| d.to_string()).collect(), ..Default::default() }
    }

    #[test]
    fn dependencies_validate_and_order() {
        let mut ws = Workspace::empty("t");
        ws.services = vec![svc("web", &["api"]), svc("api", &["db", "auth"]), svc("auth", &["db"]), svc("db", &[]), svc("x", &[])];
        ws.validate().unwrap();
        assert_eq!(ws.launch_order(&["web".into()]), vec!["db", "auth", "api", "web"]);
        assert_eq!(ws.launch_order(&["x".into(), "auth".into()]), vec!["x", "db", "auth"]);
        ws.services[3].depends_on = vec!["web".into()];
        assert_eq!(ws.validate().unwrap_err(), "의존 관계가 순환합니다: WEB → API → DB → WEB");
        ws.services[3].depends_on = vec!["db".into()];
        assert!(ws.validate().unwrap_err().contains("자기 자신"));
        ws.services[3].depends_on = vec!["nope".into()];
        assert!(ws.validate().unwrap_err().contains("'nope'"));
    }

    #[test]
    fn presets_and_restart_policy() {
        let mut ws = Workspace::empty("t");
        ws.services = vec![svc("a", &[])];
        ws.presets = vec![Preset { id: "p".into(), name: "풀스택".into(), services: vec!["a".into(), "b".into()], ..Default::default() }];
        assert!(ws.validate().unwrap_err().contains("'b'"));
        let r = AutoRestartOpts::default();
        assert_eq!((r.max(), r.delay(0).as_secs(), r.delay(1).as_secs(), r.delay(9).as_secs()), (3, 3, 6, 60));
    }

    #[test]
    fn validate_detects_missing_group_and_duplicates() {
        let mut ws = Workspace::empty("t");
        ws.services.push(Service {
            id: "a".into(),
            name: "a".into(),
            group: Some("nope".into()),
            ..Default::default()
        });
        assert!(ws.validate().is_err());
        ws.services[0].group = None;
        ws.services.push(ws.services[0].clone());
        assert!(ws.validate().unwrap_err().contains("중복"));
    }
}
