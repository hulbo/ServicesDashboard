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
        Ok(())
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
