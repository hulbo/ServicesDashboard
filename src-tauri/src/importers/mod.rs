//! 프로젝트 폴더를 스캔해 서비스 후보를 만든다.
//!
//! - IntelliJ 실행 구성: `.run/*.run.xml`, `.idea/runConfigurations/*.xml`, `.idea/workspace.xml`
//! - `package.json` scripts
//! - Spring Boot 모듈 (Gradle `settings.gradle` / Maven `pom.xml`)
//!
//! 원본 파일은 읽기만 한다.

mod intellij;
mod project;
mod scanners;
mod util;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;

use crate::launch::{self, LaunchContext};
use crate::model::{Service, ServiceKind, Workspace};
use crate::runtime::RuntimeInfo;

pub use project::Project;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Intellij,
    Npm,
    Gradle,
    Maven,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// 원본을 가리키는 고유 키 (다시 동기화할 때 같은 후보를 찾는 데 쓴다)
    pub key: String,
    pub source: SourceKind,
    /// 사람이 읽는 출처 (예: `.run/Portal.run.xml`, `front/portal/package.json`)
    pub origin: String,
    /// 그룹 이름 (IntelliJ 폴더 등)
    pub group: Option<String>,
    pub service: Service,
    /// npm 후보: 고를 수 있는 스크립트
    pub scripts: Vec<String>,
    /// Spring 후보: 프로파일 후보
    pub profiles: Vec<String>,
    pub warnings: Vec<String>,
    /// 기본으로 체크할지
    pub recommended: bool,
    pub command: Option<String>,
    pub command_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub root: String,
    pub suggested_var: String,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
}

pub struct ScanContext<'a> {
    pub runtimes: &'a [RuntimeInfo],
    pub jetbrains_dirs: &'a [PathBuf],
    pub user_home: Option<PathBuf>,
    pub gradle_init_script: &'a Path,
}

impl Candidate {
    fn new(key: String, source: SourceKind, origin: String, service: Service) -> Self {
        Candidate {
            key,
            source,
            origin,
            group: None,
            service,
            scripts: Vec::new(),
            profiles: Vec::new(),
            warnings: Vec::new(),
            recommended: false,
            command: None,
            command_error: None,
        }
    }

    /// 중복 판단용: 이름·그룹·출처를 뺀 실행 정의
    fn fingerprint(&self) -> String {
        let mut s = self.service.clone();
        s.id.clear();
        s.name.clear();
        s.extra.remove("source");
        serde_json::to_string(&s).unwrap_or_default()
    }

    /// 같은 대상을 실행하는지 (IntelliJ 구성과 스캐너 후보의 중복 제거용)
    fn target(&self) -> Option<String> {
        let s = &self.service;
        match s.kind {
            ServiceKind::GradleBoot => Some(format!(
                "gradle|{}|{}",
                s.gradle.as_ref().and_then(|g| g.project.clone()).unwrap_or_default(),
                s.main_class.clone().unwrap_or_default()
            )),
            ServiceKind::MavenBoot => Some(format!(
                "maven|{}|{}",
                s.maven.as_ref().and_then(|m| m.module.clone()).unwrap_or_default(),
                s.main_class.clone().unwrap_or_default()
            )),
            ServiceKind::Npm => s.cwd.as_ref().map(|c| format!("npm|{}", c.to_lowercase().trim_end_matches('/'))),
            _ => None,
        }
    }
}

pub fn scan(root: &Path, ctx: &ScanContext) -> Result<ScanResult, String> {
    if !root.is_dir() {
        return Err(format!("폴더가 없습니다: {}", root.display()));
    }
    let root = std::fs::canonicalize(root).map(|p| strip_verbatim(&p)).unwrap_or_else(|_| root.to_path_buf());
    let mut warnings = Vec::new();
    let project = Project::detect(&root, ctx, &mut warnings);

    let mut intellij = intellij::collect(&project, ctx, &mut warnings);
    // 같은 이름·같은 정의가 .run과 workspace.xml에 모두 있으면 하나만 남긴다
    let mut seen = HashSet::new();
    intellij.retain(|c| seen.insert(format!("{}|{}", c.service.name, c.fingerprint())));

    // IntelliJ 구성과 같은 대상을 실행하는 스캐너 후보는 뺀다.
    // 메인 클래스 없이 모듈만 지정한 IntelliJ 구성(Gradle bootRun 등)은 그 모듈 전체를 덮는다.
    let covered: HashSet<String> = intellij.iter().filter_map(Candidate::target).collect();
    let covered_modules: HashSet<String> = covered
        .iter()
        .filter_map(|t| t.rsplit_once('|').filter(|(_, main)| main.is_empty()).map(|(m, _)| m.to_string()))
        .collect();
    let mut scanned = scanners::npm(&project);
    scanned.extend(scanners::spring(&project, ctx));
    scanned.retain(|c| match c.target() {
        Some(t) => !covered.contains(&t) && !t.rsplit_once('|').is_some_and(|(m, _)| covered_modules.contains(m)),
        None => true,
    });

    let has_intellij = intellij.iter().any(|c| c.recommended);
    if has_intellij {
        for c in &mut scanned {
            c.recommended = false;
        }
    }

    let mut candidates: Vec<Candidate> = intellij.into_iter().chain(scanned).collect();
    let root_str = util::to_slash(&root);
    for c in &mut candidates {
        c.service.extra.insert(
            "source".into(),
            json!({ "kind": c.source, "root": root_str, "key": c.key, "origin": c.origin }),
        );
        preview(c, &root, ctx);
    }

    Ok(ScanResult { suggested_var: util::var_name_for(&root), root: root_str, candidates, warnings })
}

/// 후보의 실제 실행 명령을 미리 계산한다 (오류도 그대로 보여 준다).
fn preview(c: &mut Candidate, root: &Path, ctx: &ScanContext) {
    let ws = Workspace::empty("scan");
    let result = launch::build(
        &LaunchContext { workspace: &ws, workspace_dir: root, gradle_init_script: ctx.gradle_init_script, runtimes: ctx.runtimes },
        &c.service,
    );
    match result {
        Ok(spec) => c.command = Some(spec.display),
        Err(e) => c.command_error = Some(e),
    }
}

/// Windows canonicalize가 붙이는 `\\?\` 접두사를 뗀다.
fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string())
}

#[cfg(test)]
mod tests;
