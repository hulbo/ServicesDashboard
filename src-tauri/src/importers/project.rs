//! 프로젝트 구조 파악: Gradle/Maven 모듈, IntelliJ 프로젝트 SDK, Node 인터프리터.

use std::path::{Path, PathBuf};

use super::util::{read_text, rel, to_slash, walk_files};
use super::ScanContext;
use crate::runtime::{self, RuntimeKind};

pub struct GradleLayout {
    pub root_name: Option<String>,
    /// settings.gradle의 include 목록 (`:apps:portal` 형태)
    pub projects: Vec<String>,
    /// 루트 빌드 파일이 Spring Boot 플러그인을 적용하는지 (subprojects/allprojects 등)
    pub boot_from_root: bool,
}

pub struct MavenModule {
    pub artifact_id: String,
    /// 루트 기준 상대 경로 (루트 자신은 빈 문자열)
    pub rel_dir: String,
    pub boot: bool,
}

pub struct Project {
    pub root: PathBuf,
    pub gradle: Option<GradleLayout>,
    pub maven: Vec<MavenModule>,
    /// IntelliJ 프로젝트 SDK를 실제 경로로 바꾼 값
    pub project_jdk: Option<String>,
    /// IntelliJ 프로젝트의 Node 인터프리터 폴더
    pub project_node: Option<String>,
    pub jdk_table: Vec<(String, PathBuf)>,
}

fn quoted_strings(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '"' || c == '\'' {
            let v: String = chars.by_ref().take_while(|&x| x != c).collect();
            out.push(v);
        }
    }
    out
}

fn first_existing(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names.iter().map(|n| dir.join(n)).find(|p| p.is_file())
}

pub fn gradle_build_file(dir: &Path) -> Option<PathBuf> {
    first_existing(dir, &["build.gradle", "build.gradle.kts"])
}

/// 빌드 스크립트가 Spring Boot 플러그인을 적용하는지 대략 판단한다.
pub fn applies_boot_plugin(text: &str) -> bool {
    const FORMS: [&str; 6] = [
        "apply plugin: 'org.springframework.boot'",
        "apply plugin: \"org.springframework.boot\"",
        "apply(plugin = \"org.springframework.boot\")",
        "id 'org.springframework.boot'",
        "id \"org.springframework.boot\"",
        "id(\"org.springframework.boot\")",
    ];
    text.lines().any(|l| !l.contains("apply false") && FORMS.iter().any(|f| l.contains(f)))
}

pub fn parse_settings(text: &str) -> (Option<String>, Vec<String>) {
    let mut root_name = None;
    let mut projects = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        if t.contains("rootProject.name") {
            root_name = quoted_strings(t).into_iter().next();
            continue;
        }
        let is_include = t.starts_with("include") && !t.starts_with("includeBuild") && !t.starts_with("includeFlat");
        if !is_include {
            continue;
        }
        let mut stmt = t.to_string();
        // include 'a',
        //         'b'  처럼 여러 줄에 걸친 경우
        while stmt.trim_end().trim_end_matches(')').trim_end().ends_with(',') {
            match lines.next() {
                Some(next) => stmt.push_str(next.trim()),
                None => break,
            }
        }
        for p in quoted_strings(&stmt) {
            let p = p.trim();
            if !p.is_empty() {
                projects.push(if p.starts_with(':') { p.to_string() } else { format!(":{p}") });
            }
        }
    }
    (root_name, projects)
}

fn maven_artifact(pom: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(pom).ok()?;
    let project = doc.root_element();
    project.children().find(|n| n.has_tag_name("artifactId")).and_then(|n| n.text()).map(|s| s.trim().to_string())
}

impl Project {
    pub fn detect(root: &Path, ctx: &ScanContext, warnings: &mut Vec<String>) -> Project {
        let gradle = first_existing(root, &["settings.gradle", "settings.gradle.kts"])
            .map(|settings| {
                let (root_name, projects) = parse_settings(&read_text(&settings).unwrap_or_default());
                (root_name, projects)
            })
            .or_else(|| gradle_build_file(root).map(|_| (None, Vec::new())))
            .map(|(root_name, projects)| {
                let boot_from_root = gradle_build_file(root)
                    .and_then(|b| read_text(&b))
                    .is_some_and(|t| applies_boot_plugin(&t));
                GradleLayout { root_name, projects, boot_from_root }
            });

        let mut maven = Vec::new();
        if gradle.is_none() && root.join("pom.xml").is_file() {
            for pom in walk_files(root, 4, |n| n == "pom.xml") {
                let Some(text) = read_text(&pom) else { continue };
                let Some(artifact_id) = maven_artifact(&text) else { continue };
                let dir = pom.parent().unwrap_or(root);
                maven.push(MavenModule {
                    artifact_id,
                    rel_dir: rel(root, dir),
                    boot: text.contains("spring-boot-maven-plugin") || text.contains("spring-boot-starter-parent"),
                });
            }
        }

        let jdk_table = runtime::intellij_jdks(ctx.jetbrains_dirs, ctx.user_home.as_deref());
        let mut project = Project { root: root.to_path_buf(), gradle, maven, project_jdk: None, project_node: None, jdk_table };

        // 프로젝트 SDK: .idea/misc.xml → 없으면 .idea/gradle.xml의 gradleJvm
        let idea = root.join(".idea");
        let sdk_name = read_text(&idea.join("misc.xml")).and_then(|t| {
            let doc = roxmltree::Document::parse(&t).ok()?;
            doc.descendants()
                .find(|n| n.attribute("name") == Some("ProjectRootManager"))
                .and_then(|n| n.attribute("project-jdk-name"))
                .map(str::to_string)
        });
        let gradle_jvm = read_text(&idea.join("gradle.xml")).and_then(|t| {
            let doc = roxmltree::Document::parse(&t).ok()?;
            doc.descendants()
                .find(|n| n.has_tag_name("option") && n.attribute("name") == Some("gradleJvm"))
                .and_then(|n| n.attribute("value"))
                .filter(|v| !v.starts_with('#'))
                .map(str::to_string)
        });
        if let Some(name) = sdk_name.or(gradle_jvm) {
            match project.resolve_jdk(&name, ctx) {
                Some(path) => project.project_jdk = Some(path),
                None => warnings.push(format!(
                    "IntelliJ 프로젝트 SDK '{name}'를 이 PC에서 찾지 못했습니다. 가져온 Java 서비스의 JDK는 시스템 JAVA_HOME을 씁니다."
                )),
            }
        }

        // Node 인터프리터: workspace.xml의 PropertiesComponent
        if let Some(text) = read_text(&idea.join("workspace.xml")) {
            project.project_node = node_interpreter(&text);
        }
        project
    }

    /// JDK 참조(경로, IntelliJ SDK 이름, 버전 별칭)를 실제 경로로 바꾼다.
    pub fn resolve_jdk(&self, value: &str, ctx: &ScanContext) -> Option<String> {
        let v = value.trim();
        if v.is_empty() {
            return None;
        }
        let p = Path::new(v);
        if p.join("bin").is_dir() {
            return Some(to_slash(p));
        }
        if let Some((_, home)) = self.jdk_table.iter().find(|(n, h)| n == v && h.join("bin").is_dir()) {
            return Some(to_slash(home));
        }
        runtime::resolve(RuntimeKind::Jdk, v, ctx.runtimes).ok()
    }

    /// IntelliJ Gradle 모듈 이름(`root.apps.portal.main`)을 프로젝트 경로(`:apps:portal`)로 바꾼다.
    pub fn gradle_path_for_module(&self, module: &str) -> String {
        let mut parts: Vec<&str> = module.split('.').collect();
        if parts.len() > 1 && matches!(*parts.last().unwrap(), "main" | "test" | "testFixtures" | "integrationTest") {
            parts.pop();
        }
        let root_name = self.gradle.as_ref().and_then(|g| g.root_name.as_deref());
        match root_name {
            Some(rn) if parts.first() == Some(&rn) => {
                parts.remove(0);
            }
            Some(_) => {
                // 루트 이름이 다르면 include 목록에서 끝이 맞는 것을 찾는다
                let tail = format!(":{}", parts.join(":"));
                if let Some(found) = self.gradle.as_ref().and_then(|g| g.projects.iter().find(|p| tail.ends_with(p.as_str()))) {
                    return found.clone();
                }
                if parts.len() > 1 {
                    parts.remove(0);
                }
            }
            None if parts.len() > 1 => {
                parts.remove(0);
            }
            None => parts.clear(),
        }
        if parts.is_empty() { String::new() } else { format!(":{}", parts.join(":")) }
    }

    pub fn gradle_project_dir(&self, path: &str) -> PathBuf {
        let rel = path.trim_start_matches(':').replace(':', "/");
        if rel.is_empty() { self.root.clone() } else { self.root.join(rel) }
    }

    pub fn maven_module(&self, artifact: &str) -> Option<&MavenModule> {
        self.maven.iter().find(|m| m.artifact_id == artifact)
    }
}

fn node_interpreter(workspace_xml: &str) -> Option<String> {
    // 새 형식: "nodejs_interpreter_path": "C:/Program Files/nodejs/node"
    // 옛 형식: <property name="nodejs_interpreter_path" value="..." />
    let key = "nodejs_interpreter_path";
    let pos = workspace_xml.find(key)?;
    let rest = &workspace_xml[pos + key.len()..];
    let value = if let Some(v) = rest.trim_start().strip_prefix("\":") {
        quoted_strings(v).into_iter().next()?
    } else {
        let start = rest.find("value=\"")? + 7;
        rest[start..].split('"').next()?.to_string()
    };
    let value = value.replace("\\\\", "/").replace('\\', "/");
    let path = PathBuf::from(&value);
    let dir = if path.is_dir() {
        path
    } else if path.is_file() || path.with_extension("exe").is_file() {
        path.parent()?.to_path_buf()
    } else {
        return None;
    };
    Some(to_slash(&dir))
}

#[cfg(test)]
pub(super) mod test_support {
    pub use super::{applies_boot_plugin, parse_settings};
}
