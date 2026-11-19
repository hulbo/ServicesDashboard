//! VS Code `.vscode/launch.json` 변환 (node, npm, node-terminal, java, compounds).

use std::path::Path;

use serde_json::Value;

use super::project::{applies_boot_plugin, gradle_build_file, Project};
use super::util::{read_text, split_args, to_slash};
use super::{Candidate, RawPreset, SourceKind};
use crate::model::{
    ClasspathSource, EnvMap, GradleOpts, JavaMainOpts, MavenOpts, NodeOpts, NpmOpts, PackageManager, Service, ServiceKind,
    ShellKind, ShellOpts,
};

const ORIGIN: &str = ".vscode/launch.json";

/// JSONC(주석·끝 쉼표 허용)를 일반 JSON으로 바꾼다.
pub fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut in_str = false;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match (c, chars.get(i + 1)) {
            ('"', _) => {
                in_str = true;
                out.push(c);
                i += 1;
            }
            ('/', Some('/')) => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
            }
            (',', _) => {
                // 뒤따르는 공백 다음이 ] 또는 }면 끝 쉼표라 버린다
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if !matches!(chars.get(j), Some(']') | Some('}')) {
                    out.push(c);
                }
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn expand(s: &str, root: &str) -> String {
    let base = Path::new(root).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    s.replace("${workspaceFolder}", root)
        .replace("${workspaceRoot}", root)
        .replace("${workspaceFolderBasename}", &base)
        .replace("${userHome}", "${userHome}")
        .replace('\\', "/")
}

fn str_or_list(v: Option<&Value>, root: &str) -> Vec<String> {
    match v {
        Some(Value::String(s)) => split_args(&expand(s, root)),
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str()).map(|x| expand(x, root)).collect(),
        _ => Vec::new(),
    }
}

fn env_of(conf: &Value, root: &str) -> EnvMap {
    let mut env = EnvMap::new();
    if let Some(obj) = conf.get("env").and_then(|e| e.as_object()) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                env.insert(k.clone(), expand(s, root));
            }
        }
    }
    env
}

pub fn collect(project: &Project, warnings: &mut Vec<String>) -> (Vec<Candidate>, Vec<RawPreset>) {
    let file = project.root.join(".vscode").join("launch.json");
    let Some(text) = read_text(&file) else { return (Vec::new(), Vec::new()) };
    let json: Value = match serde_json::from_str(&strip_jsonc(&text)) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("{ORIGIN}을(를) 해석하지 못했습니다: {e}"));
            return (Vec::new(), Vec::new());
        }
    };
    let root = to_slash(&project.root);
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for conf in json.get("configurations").and_then(|c| c.as_array()).into_iter().flatten() {
        let name = conf.get("name").and_then(|n| n.as_str()).unwrap_or("이름 없음").to_string();
        let kind = conf.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let request = conf.get("request").and_then(|t| t.as_str()).unwrap_or("launch");
        if request != "launch" {
            skipped.push(format!("{name} (attach)"));
            continue;
        }
        let cwd = conf.get("cwd").and_then(|c| c.as_str()).map(|c| expand(c, &root)).unwrap_or_else(|| root.clone());
        let mut svc = Service { name: name.clone(), cwd: Some(cwd), env: env_of(conf, &root), ..Default::default() };
        if let Some(f) = conf.get("envFile").and_then(|f| f.as_str()) {
            svc.env_files = vec![expand(f, &root)];
        }
        let mut c_warnings = Vec::new();
        match kind {
            "node" | "pwa-node" => {
                let runtime = conf.get("runtimeExecutable").and_then(|r| r.as_str()).unwrap_or("node");
                let runtime_args = str_or_list(conf.get("runtimeArgs"), &root);
                let pm = match runtime {
                    "npm" => Some(PackageManager::Npm),
                    "yarn" => Some(PackageManager::Yarn),
                    "pnpm" => Some(PackageManager::Pnpm),
                    _ => None,
                };
                if let Some(pm) = pm {
                    let script = match runtime_args.as_slice() {
                        [cmd, s, ..] if cmd == "run" || cmd == "run-script" => s.clone(),
                        [s, ..] if pm == PackageManager::Yarn => s.clone(),
                        _ => {
                            skipped.push(format!("{name} (npm 스크립트를 알 수 없음)"));
                            continue;
                        }
                    };
                    let skip = if pm == PackageManager::Yarn && runtime_args.first().map(String::as_str) != Some("run") { 1 } else { 2 };
                    svc.args = runtime_args.iter().skip(skip).filter(|a| *a != "--").cloned().collect();
                    svc.kind = ServiceKind::Npm;
                    svc.npm = Some(NpmOpts { script, package_manager: pm, ..Default::default() });
                } else if let Some(program) = conf.get("program").and_then(|p| p.as_str()) {
                    svc.kind = ServiceKind::Node;
                    svc.node = Some(NodeOpts { entry: expand(program, &root), node_args: runtime_args, ..Default::default() });
                    svc.args = str_or_list(conf.get("args"), &root);
                } else {
                    skipped.push(format!("{name} (실행할 program 없음)"));
                    continue;
                }
            }
            "node-terminal" => {
                let Some(cmd) = conf.get("command").and_then(|c| c.as_str()) else {
                    skipped.push(format!("{name} (command 없음)"));
                    continue;
                };
                svc.kind = ServiceKind::Shell;
                svc.shell = Some(ShellOpts { command: expand(cmd, &root), shell: ShellKind::Cmd, ..Default::default() });
            }
            "java" => {
                let Some(main) = conf.get("mainClass").and_then(|m| m.as_str()) else {
                    skipped.push(format!("{name} (mainClass 없음)"));
                    continue;
                };
                svc.main_class = Some(main.to_string());
                svc.jvm_args = str_or_list(conf.get("vmArgs"), &root);
                svc.args = str_or_list(conf.get("args"), &root);
                let project_name = conf.get("projectName").and_then(|p| p.as_str()).unwrap_or("");
                map_java(&mut svc, project, project_name, &mut c_warnings);
            }
            other => {
                skipped.push(format!("{name} (지원하지 않는 타입 {other})"));
                continue;
            }
        }
        let mut c = Candidate::new(format!("vscode:{name}"), SourceKind::Vscode, ORIGIN.into(), svc);
        c.warnings = c_warnings;
        c.recommended = true;
        out.push(c);
    }
    let mut presets = Vec::new();
    for comp in json.get("compounds").and_then(|c| c.as_array()).into_iter().flatten() {
        let name = comp.get("name").and_then(|n| n.as_str()).unwrap_or("compound").to_string();
        let members = comp
            .get("configurations")
            .and_then(|c| c.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        presets.push(RawPreset { key: format!("vscode-compound:{name}"), name, origin: ORIGIN.into(), members, source: SourceKind::Vscode });
    }
    if !skipped.is_empty() {
        warnings.push(format!("가져오지 않은 VS Code 구성: {}", skipped.join(", ")));
    }
    (out, presets)
}

/// VS Code java 구성의 projectName을 Gradle/Maven 모듈로 맞춘다.
fn map_java(svc: &mut Service, project: &Project, project_name: &str, warnings: &mut Vec<String>) {
    if let Some(gradle) = &project.gradle {
        let path = gradle
            .projects
            .iter()
            .find(|p| !project_name.is_empty() && p.rsplit(':').next() == Some(project_name))
            .cloned()
            .unwrap_or_default();
        let dir = project.gradle_project_dir(&path);
        let boot = gradle.boot_from_root || gradle_build_file(&dir).and_then(|b| read_text(&b)).is_some_and(|t| applies_boot_plugin(&t));
        if boot {
            svc.kind = ServiceKind::GradleBoot;
            svc.gradle = Some(GradleOpts { project: (!path.is_empty()).then_some(path), ..Default::default() });
        } else {
            svc.kind = ServiceKind::JavaMain;
            svc.java_main = Some(JavaMainOpts { source: ClasspathSource::Gradle, module: (!path.is_empty()).then_some(path), ..Default::default() });
        }
    } else if !project.maven.is_empty() {
        let found = project.maven_module(project_name);
        let module = found.map(|m| m.rel_dir.clone()).filter(|r| !r.is_empty());
        if found.is_some_and(|m| m.boot) {
            svc.kind = ServiceKind::MavenBoot;
            svc.maven = Some(MavenOpts { module, ..Default::default() });
        } else {
            svc.kind = ServiceKind::JavaMain;
            svc.java_main = Some(JavaMainOpts { source: ClasspathSource::Maven, module, ..Default::default() });
        }
    } else {
        svc.kind = ServiceKind::JavaMain;
        svc.java_main = Some(JavaMainOpts { source: ClasspathSource::Manual, ..Default::default() });
        warnings.push("Gradle/Maven 프로젝트가 아니라 클래스패스를 직접 입력해야 합니다.".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonc() {
        let t = r#"{
          // 주석
          "a": "http://x//y", /* 블록 */
          "b": [1, 2,],
        }"#;
        let v: Value = serde_json::from_str(&strip_jsonc(t)).unwrap();
        assert_eq!(v["a"], "http://x//y");
        assert_eq!(v["b"], serde_json::json!([1, 2]));
    }
}
