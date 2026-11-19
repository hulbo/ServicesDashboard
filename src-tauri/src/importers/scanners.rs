//! IntelliJ 구성이 없어도 찾을 수 있는 후보: package.json scripts, Spring Boot 모듈.

use std::path::Path;

use super::project::{applies_boot_plugin, gradle_build_file, Project};
use super::util::{read_text, rel, spring_main_classes, spring_profiles, to_slash, walk_files};
use super::{Candidate, ScanContext, SourceKind};
use crate::model::{GradleOpts, MavenOpts, NpmOpts, PackageManager, RuntimeOpts, Service, ServiceKind, SpringOpts};

/// 서비스로 띄울 만한 스크립트 (앞쪽이 우선)
const PREFERRED_SCRIPTS: [&str; 4] = ["dev", "start", "serve", "start:dev"];

pub fn package_scripts(package_json: &Path) -> Vec<String> {
    let Some(text) = read_text(package_json) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
    v.get("scripts").and_then(|s| s.as_object()).map(|m| m.keys().cloned().collect()).unwrap_or_default()
}

fn package_manager(dir: &Path, root: &Path) -> PackageManager {
    let mut cur = Some(dir);
    while let Some(d) = cur {
        if d.join("pnpm-lock.yaml").is_file() {
            return PackageManager::Pnpm;
        }
        if d.join("yarn.lock").is_file() {
            return PackageManager::Yarn;
        }
        if d.join("package-lock.json").is_file() || d == root {
            break;
        }
        cur = d.parent();
    }
    PackageManager::Npm
}

pub fn npm(project: &Project) -> Vec<Candidate> {
    let root = &project.root;
    let mut out = Vec::new();
    for pkg in walk_files(root, 4, |n| n == "package.json") {
        let Some(text) = read_text(&pkg) else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let scripts: Vec<String> =
            json.get("scripts").and_then(|s| s.as_object()).map(|m| m.keys().cloned().collect()).unwrap_or_default();
        if scripts.is_empty() {
            continue;
        }
        let dir = pkg.parent().unwrap_or(root);
        let preferred = PREFERRED_SCRIPTS.iter().find(|p| scripts.iter().any(|s| s == *p)).map(|s| s.to_string());
        let script = preferred.clone().unwrap_or_else(|| scripts[0].clone());
        let dir_name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "app".into());
        let name = json
            .get("name")
            .and_then(|n| n.as_str())
            .map(|n| n.rsplit('/').next().unwrap_or(n).to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or(dir_name);
        let rel_dir = rel(root, dir);

        let svc = Service {
            name: name.clone(),
            kind: ServiceKind::Npm,
            cwd: Some(to_slash(dir)),
            npm: Some(NpmOpts { script, package_manager: package_manager(dir, root), ..Default::default() }),
            runtime: project.project_node.clone().map(|n| RuntimeOpts { node: Some(n), ..Default::default() }),
            ..Default::default()
        };
        let origin = if rel_dir.is_empty() { "package.json".to_string() } else { format!("{rel_dir}/package.json") };
        let mut c = Candidate::new(format!("npm:{rel_dir}"), SourceKind::Npm, origin, svc);
        c.group = Some("npm".into());
        c.scripts = scripts;
        c.recommended = preferred.is_some();
        out.push(c);
    }
    out
}

fn spring_candidate(
    project: &Project,
    kind: ServiceKind,
    module_key: &str,
    module_dir: &Path,
    main: &str,
    multiple: bool,
) -> Candidate {
    let simple = main.rsplit('.').next().unwrap_or(main).to_string();
    let profiles = spring_profiles(module_dir);
    let mut svc = Service {
        name: simple,
        kind,
        cwd: Some(to_slash(&project.root)),
        main_class: Some(main.to_string()),
        runtime: project.project_jdk.clone().map(|j| RuntimeOpts { jdk: Some(j), ..Default::default() }),
        ..Default::default()
    };
    // 로컬 개발용 프로파일이 있으면 기본으로 고른다
    if let Some(p) = ["local", "dev"].iter().find(|p| profiles.iter().any(|x| x == *p)) {
        svc.spring = Some(SpringOpts { profiles: vec![p.to_string()], ..Default::default() });
    }
    let (key, origin) = match kind {
        ServiceKind::GradleBoot => {
            svc.gradle = Some(GradleOpts { project: (!module_key.is_empty()).then(|| module_key.to_string()), ..Default::default() });
            (format!("gradle:{module_key}#{main}"), format!("Gradle {}", if module_key.is_empty() { ":" } else { module_key }))
        }
        _ => {
            svc.maven = Some(MavenOpts { module: (!module_key.is_empty()).then(|| module_key.to_string()), ..Default::default() });
            (format!("maven:{module_key}#{main}"), format!("Maven {}", if module_key.is_empty() { "(루트)" } else { module_key }))
        }
    };
    let mut c = Candidate::new(key, if kind == ServiceKind::GradleBoot { SourceKind::Gradle } else { SourceKind::Maven }, origin, svc);
    c.group = Some("server".into());
    c.profiles = profiles;
    c.recommended = true;
    if multiple {
        c.warnings.push("이 모듈에 @SpringBootApplication 클래스가 여러 개 있습니다.".into());
    }
    c
}

pub fn spring(project: &Project, _ctx: &ScanContext) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Some(gradle) = &project.gradle {
        let mut paths: Vec<String> = vec![String::new()];
        paths.extend(gradle.projects.iter().cloned());
        for path in paths {
            let dir = project.gradle_project_dir(&path);
            if !dir.is_dir() {
                continue;
            }
            let module_boot = gradle_build_file(&dir).and_then(|b| read_text(&b)).is_some_and(|t| {
                applies_boot_plugin(&t) || t.contains("org.springframework.boot")
            });
            if !(module_boot || gradle.boot_from_root) {
                continue;
            }
            let mains = spring_main_classes(&dir);
            for main in &mains {
                out.push(spring_candidate(project, ServiceKind::GradleBoot, &path, &dir, main, mains.len() > 1));
            }
        }
    }
    for m in project.maven.iter().filter(|m| m.boot) {
        let dir = if m.rel_dir.is_empty() { project.root.clone() } else { project.root.join(&m.rel_dir) };
        let mains = spring_main_classes(&dir);
        for main in &mains {
            out.push(spring_candidate(project, ServiceKind::MavenBoot, &m.rel_dir, &dir, main, mains.len() > 1));
        }
    }
    out
}
