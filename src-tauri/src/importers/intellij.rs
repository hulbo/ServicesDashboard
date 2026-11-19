//! IntelliJ 실행 구성(.run/*.run.xml, .idea/runConfigurations, .idea/workspace.xml) 변환.

use std::path::{Path, PathBuf};

use roxmltree::{Document, Node};

use super::project::{applies_boot_plugin, gradle_build_file, Project};
use super::util::{port_from_args, read_text, rel, split_args, to_slash};
use super::{Candidate, ScanContext, SourceKind};
use crate::model::{
    EnvMap, GradleOpts, JarOpts, MavenOpts, NodeOpts, NpmOpts, PackageManager, RuntimeOpts, Service, ServiceKind, ShellKind,
    ShellOpts, SpringOpts,
};

const SPRING_BOOT: &str = "SpringBootApplicationConfigurationType";

pub fn collect(project: &Project, ctx: &ScanContext, warnings: &mut Vec<String>) -> Vec<Candidate> {
    let root = &project.root;
    let mut files: Vec<(PathBuf, bool)> = Vec::new();
    for dir in [root.join(".run"), root.join(".idea").join("runConfigurations")] {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut v: Vec<PathBuf> =
                rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "xml")).collect();
            v.sort();
            files.extend(v.into_iter().map(|p| (p, false)));
        }
    }
    let ws = root.join(".idea").join("workspace.xml");
    if ws.is_file() {
        files.push((ws, true));
    }

    let mut out = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for (file, is_workspace) in files {
        let origin = rel(root, &file);
        let Some(text) = read_text(&file) else { continue };
        let doc = match Document::parse(&text) {
            Ok(d) => d,
            Err(e) => {
                warnings.push(format!("{origin}을(를) 해석하지 못했습니다: {e}"));
                continue;
            }
        };
        let confs: Vec<Node> = if is_workspace {
            doc.descendants()
                .filter(|n| n.has_tag_name("component") && n.attribute("name") == Some("RunManager"))
                .flat_map(|rm| rm.children().filter(|c| c.has_tag_name("configuration")))
                .collect()
        } else {
            doc.descendants().filter(|n| n.has_tag_name("configuration")).collect()
        };
        for conf in confs {
            if conf.attribute("default") == Some("true") {
                continue; // 템플릿
            }
            let name = conf.attribute("name").unwrap_or("이름 없음").to_string();
            let kind = conf.attribute("type").unwrap_or("");
            let mut c = match convert(conf, kind, &name, project, ctx) {
                Converted::Ok(c) => *c,
                Converted::Skip(reason) => {
                    skipped.push(format!("{name} ({reason})"));
                    continue;
                }
            };
            c.key = format!("intellij:{origin}#{name}");
            c.origin = origin.clone();
            c.group = conf.attribute("folderName").map(str::to_string);
            let temporary = conf.attribute("temporary") == Some("true");
            if temporary {
                c.warnings.push("IntelliJ의 임시 구성입니다.".into());
            }
            c.recommended = !temporary && c.service.kind != ServiceKind::JavaMain;
            out.push(c);
        }
    }
    if !skipped.is_empty() {
        warnings.push(format!("가져오지 않은 IntelliJ 구성: {}", skipped.join(", ")));
    }
    out
}

enum Converted {
    Ok(Box<Candidate>),
    Skip(String),
}

// ---------------------------------------------------------------- XML 도우미

/// `<option name="X" value="Y"/>` (직계 자식)
fn opt(conf: Node, name: &str) -> Option<String> {
    conf.children()
        .find(|n| n.has_tag_name("option") && n.attribute("name") == Some(name))
        .and_then(|n| n.attribute("value"))
        .map(str::to_string)
        .filter(|v| !v.trim().is_empty())
}

/// `<option name="X" value="Y"/>` (자손 어디든)
fn deep_opt(conf: Node, name: &str) -> Option<String> {
    conf.descendants()
        .find(|n| n.has_tag_name("option") && n.attribute("name") == Some(name))
        .and_then(|n| n.attribute("value"))
        .map(str::to_string)
        .filter(|v| !v.trim().is_empty())
}

/// `<option name="X"><list><option value="a"/>...</list></option>`
fn deep_list(conf: Node, name: &str) -> Vec<String> {
    conf.descendants()
        .find(|n| n.has_tag_name("option") && n.attribute("name") == Some(name))
        .map(|o| {
            o.descendants()
                .filter(|n| n.has_tag_name("option") && n.attribute("name").is_none())
                .filter_map(|n| n.attribute("value").map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// `<tag value="X"/>` (직계 자식, npm 구성 형식)
fn child_value(conf: Node, tag: &str) -> Option<String> {
    conf.children()
        .find(|n| n.has_tag_name(tag))
        .and_then(|n| n.attribute("value"))
        .map(str::to_string)
        .filter(|v| !v.trim().is_empty())
}

/// `<envs><env name="A" value="B"/></envs>` 또는 `<option name="env"><map><entry key="A" value="B"/></map></option>`
fn envs(conf: Node) -> EnvMap {
    let mut map = EnvMap::new();
    if let Some(envs) = conf.children().find(|n| n.has_tag_name("envs")) {
        for e in envs.children().filter(|n| n.has_tag_name("env")) {
            if let (Some(k), Some(v)) = (e.attribute("name"), e.attribute("value")) {
                map.insert(k.to_string(), v.to_string());
            }
        }
    }
    if let Some(o) = conf.descendants().find(|n| n.has_tag_name("option") && n.attribute("name") == Some("env")) {
        for e in o.descendants().filter(|n| n.has_tag_name("entry")) {
            if let (Some(k), Some(v)) = (e.attribute("key"), e.attribute("value")) {
                map.insert(k.to_string(), v.to_string());
            }
        }
    }
    map
}

fn expand_macros(s: &str, project: &Project, module_dir: &Path, ctx: &ScanContext) -> String {
    let root = to_slash(&project.root);
    let module = to_slash(module_dir);
    let home = ctx.user_home.as_deref().map(to_slash).unwrap_or_default();
    s.replace("$PROJECT_DIR$", &root)
        .replace("$MODULE_WORKING_DIR$", &module)
        .replace("$MODULE_DIR$", &module)
        .replace("$USER_HOME$", &home)
        .replace('\\', "/")
}

fn base_service(name: &str, kind: ServiceKind) -> Service {
    Service { id: String::new(), name: name.to_string(), kind, ..Default::default() }
}

fn candidate(name: &str, svc: Service) -> Candidate {
    Candidate::new(String::new(), SourceKind::Intellij, String::new(), {
        let mut s = svc;
        s.name = name.to_string();
        s
    })
}

// ---------------------------------------------------------------- 변환

fn convert(conf: Node, kind: &str, name: &str, project: &Project, ctx: &ScanContext) -> Converted {
    match kind {
        SPRING_BOOT | "Application" => java_main(conf, kind == SPRING_BOOT, name, project, ctx),
        "js.build_tools.npm" => npm(conf, name, project, ctx),
        "NodeJSConfigurationType" => node(conf, name, project, ctx),
        "JarApplication" => jar(conf, name, project, ctx),
        "MavenRunConfiguration" => maven(conf, name, project, ctx),
        "GradleRunConfiguration" => gradle(conf, name, project, ctx),
        "ShConfigurationType" => shell(conf, name, project, ctx),
        "CompoundRunConfigurationType" => Converted::Skip("복합 구성은 M4 실행 프리셋에서 지원 예정".into()),
        other => Converted::Skip(format!("지원하지 않는 타입 {other}")),
    }
}

/// 구성의 JRE 설정 → JDK 경로. 대체 JRE가 꺼져 있으면 프로젝트 SDK.
fn config_jdk(conf: Node, project: &Project, ctx: &ScanContext, warnings: &mut Vec<String>) -> Option<String> {
    if opt(conf, "ALTERNATIVE_JRE_PATH_ENABLED").as_deref() == Some("true") {
        if let Some(v) = opt(conf, "ALTERNATIVE_JRE_PATH") {
            let resolved = project.resolve_jdk(&v, ctx);
            if resolved.is_none() {
                warnings.push(format!("대체 JRE '{v}'를 찾지 못해 프로젝트 SDK를 씁니다."));
            } else {
                return resolved;
            }
        }
    }
    project.project_jdk.clone()
}

fn java_main(conf: Node, is_boot_config: bool, name: &str, project: &Project, ctx: &ScanContext) -> Converted {
    let mut warnings = Vec::new();
    let main = opt(conf, "SPRING_BOOT_MAIN_CLASS").or_else(|| opt(conf, "MAIN_CLASS_NAME"));
    let module = conf.children().find(|n| n.has_tag_name("module")).and_then(|n| n.attribute("name")).map(str::to_string);
    let mut args = opt(conf, "PROGRAM_PARAMETERS").map(|p| split_args(&p)).unwrap_or_default();
    // "구성 속성 재정의" (Spring Boot 구성의 additionalParameters)
    if let Some(params) = conf.children().find(|n| n.has_tag_name("additionalParameters")) {
        for p in params.children().filter(|n| n.has_tag_name("param")) {
            if opt(p, "enabled").as_deref() == Some("false") {
                continue;
            }
            if let (Some(k), Some(v)) = (opt(p, "name"), opt(p, "value")) {
                args.push(format!("--{k}={v}"));
            }
        }
    }
    let jvm_args = opt(conf, "VM_PARAMETERS").or_else(|| opt(conf, "VM_OPTIONS")).map(|p| split_args(&p)).unwrap_or_default();
    let profiles: Vec<String> = opt(conf, "ACTIVE_PROFILES")
        .map(|p| p.split([',', ' ']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect())
        .unwrap_or_default();

    let mut svc = base_service(name, ServiceKind::JavaMain);
    svc.cwd = Some(to_slash(&project.root));
    svc.main_class = main.clone();
    svc.env = envs(conf);
    svc.jvm_args = jvm_args;
    if let Some(p) = port_from_args(&args) {
        svc.ports = vec![p];
    }
    svc.args = args;
    if !profiles.is_empty() {
        svc.spring = Some(SpringOpts { profiles, ..Default::default() });
    }
    if let Some(jdk) = config_jdk(conf, project, ctx, &mut warnings) {
        svc.runtime = Some(RuntimeOpts { jdk: Some(jdk), ..Default::default() });
    }

    if let Some(gradle) = &project.gradle {
        let path = module.as_deref().map(|m| project.gradle_path_for_module(m)).unwrap_or_default();
        let dir = project.gradle_project_dir(&path);
        if !dir.is_dir() {
            warnings.push(format!("모듈 폴더를 찾지 못했습니다: {}", rel(&project.root, &dir)));
        }
        let module_boot = gradle_build_file(&dir).and_then(|b| read_text(&b)).is_some_and(|t| applies_boot_plugin(&t));
        if is_boot_config || gradle.boot_from_root || module_boot {
            svc.kind = ServiceKind::GradleBoot;
            svc.gradle = Some(GradleOpts { project: (!path.is_empty()).then_some(path), ..Default::default() });
        } else {
            svc.kind = ServiceKind::Gradle;
            svc.gradle = Some(GradleOpts {
                project: (!path.is_empty()).then_some(path),
                tasks: vec!["run".into()],
                ..Default::default()
            });
            warnings.push("Spring Boot 플러그인이 없는 모듈이라 Gradle run 태스크로 바꿨습니다 (application 플러그인 필요).".into());
        }
    } else if !project.maven.is_empty() {
        let found = module.as_deref().and_then(|m| project.maven_module(m));
        if module.is_some() && found.is_none() {
            warnings.push(format!("Maven 모듈 '{}'을(를) 찾지 못해 루트에서 실행합니다.", module.unwrap_or_default()));
        }
        svc.kind = ServiceKind::MavenBoot;
        svc.maven = Some(MavenOpts {
            module: found.map(|m| m.rel_dir.clone()).filter(|r| !r.is_empty()),
            ..Default::default()
        });
        if !is_boot_config && !found.is_some_and(|m| m.boot) {
            warnings.push("Spring Boot 모듈인지 확인하지 못했습니다. spring-boot:run이 실패하면 타입을 바꾸세요.".into());
        }
    } else {
        warnings.push("Gradle/Maven 프로젝트가 아니라 메인 클래스를 직접 실행해야 합니다 (java-main, M4 예정).".into());
    }

    let mut c = candidate(name, svc);
    c.warnings = warnings;
    Converted::Ok(Box::new(c))
}

fn npm(conf: Node, name: &str, project: &Project, ctx: &ScanContext) -> Converted {
    let Some(pkg) = child_value(conf, "package-json") else {
        return Converted::Skip("package.json 경로 없음".into());
    };
    let pkg = PathBuf::from(expand_macros(&pkg, project, &project.root, ctx));
    let dir = pkg.parent().map(Path::to_path_buf).unwrap_or_else(|| project.root.clone());
    let command = child_value(conf, "command").unwrap_or_else(|| "run".into());
    let scripts: Vec<String> = conf
        .children()
        .filter(|n| n.has_tag_name("scripts"))
        .flat_map(|s| s.children().filter(|n| n.has_tag_name("script")))
        .filter_map(|n| n.attribute("value").map(str::to_string))
        .collect();
    let script = match command.as_str() {
        "run" | "run-script" => match scripts.first() {
            Some(s) => s.clone(),
            None => return Converted::Skip("실행할 스크립트 없음".into()),
        },
        "start" | "test" => command.clone(),
        other => return Converted::Skip(format!("npm {other} 명령은 서비스가 아님")),
    };
    let pm_value = child_value(conf, "package-manager").unwrap_or_default().to_lowercase();
    let package_manager = if pm_value.contains("pnpm") {
        PackageManager::Pnpm
    } else if pm_value.contains("yarn") {
        PackageManager::Yarn
    } else {
        PackageManager::Npm
    };
    let node_dir = match child_value(conf, "node-interpreter").as_deref() {
        None | Some("project") => project.project_node.clone(),
        Some(p) => {
            let p = PathBuf::from(expand_macros(p, project, &project.root, ctx));
            if p.is_dir() { Some(to_slash(&p)) } else { p.parent().filter(|d| d.is_dir()).map(to_slash) }
        }
    };
    let args = child_value(conf, "arguments").map(|a| split_args(&a)).unwrap_or_default();

    let mut svc = base_service(name, ServiceKind::Npm);
    svc.cwd = Some(to_slash(&dir));
    svc.npm = Some(NpmOpts { script, package_manager, ..Default::default() });
    svc.env = envs(conf);
    if let Some(p) = port_from_args(&args) {
        svc.ports = vec![p];
    }
    svc.args = args;
    if let Some(node) = node_dir {
        svc.runtime = Some(RuntimeOpts { node: Some(node), ..Default::default() });
    }
    let mut c = candidate(name, svc);
    c.scripts = super::scanners::package_scripts(&pkg);
    Converted::Ok(Box::new(c))
}

fn node(conf: Node, name: &str, project: &Project, ctx: &ScanContext) -> Converted {
    let attr = |k: &str| conf.attribute(k).filter(|v| !v.trim().is_empty()).map(str::to_string);
    let Some(entry) = attr("path-to-js-file") else {
        return Converted::Skip("스크립트 파일 없음".into());
    };
    let cwd = attr("working-dir").map(|w| expand_macros(&w, project, &project.root, ctx)).unwrap_or_else(|| to_slash(&project.root));
    let mut svc = base_service(name, ServiceKind::Node);
    svc.cwd = Some(cwd);
    svc.node = Some(NodeOpts {
        entry: expand_macros(&entry, project, &project.root, ctx),
        node_args: attr("node-parameters").map(|a| split_args(&a)).unwrap_or_default(),
        ..Default::default()
    });
    svc.args = attr("application-parameters").map(|a| split_args(&a)).unwrap_or_default();
    svc.env = envs(conf);
    let node_dir = attr("path-to-node").and_then(|p| {
        let p = PathBuf::from(expand_macros(&p, project, &project.root, ctx));
        if p.is_dir() { Some(to_slash(&p)) } else { p.parent().filter(|d| d.is_dir()).map(to_slash) }
    });
    if let Some(n) = node_dir.or_else(|| project.project_node.clone()) {
        svc.runtime = Some(RuntimeOpts { node: Some(n), ..Default::default() });
    }
    Converted::Ok(Box::new(candidate(name, svc)))
}

fn jar(conf: Node, name: &str, project: &Project, ctx: &ScanContext) -> Converted {
    let mut warnings = Vec::new();
    let Some(jar_path) = opt(conf, "JAR_PATH") else {
        return Converted::Skip("jar 경로 없음".into());
    };
    let mut svc = base_service(name, ServiceKind::Jar);
    svc.cwd = Some(
        opt(conf, "WORKING_DIRECTORY")
            .map(|w| expand_macros(&w, project, &project.root, ctx))
            .unwrap_or_else(|| to_slash(&project.root)),
    );
    svc.jar = Some(JarOpts { path: expand_macros(&jar_path, project, &project.root, ctx), ..Default::default() });
    svc.jvm_args = opt(conf, "VM_PARAMETERS").map(|p| split_args(&p)).unwrap_or_default();
    svc.args = opt(conf, "PROGRAM_PARAMETERS").map(|p| split_args(&p)).unwrap_or_default();
    svc.env = envs(conf);
    if let Some(jdk) = config_jdk(conf, project, ctx, &mut warnings) {
        svc.runtime = Some(RuntimeOpts { jdk: Some(jdk), ..Default::default() });
    }
    let mut c = candidate(name, svc);
    c.warnings = warnings;
    Converted::Ok(Box::new(c))
}

fn maven(conf: Node, name: &str, project: &Project, ctx: &ScanContext) -> Converted {
    let goals = deep_list(conf, "goals");
    if goals.is_empty() {
        return Converted::Skip("Maven 골 없음".into());
    }
    let cwd = deep_opt(conf, "workingDirPath")
        .map(|w| expand_macros(&w, project, &project.root, ctx))
        .unwrap_or_else(|| to_slash(&project.root));
    let mut options = deep_opt(conf, "cmdOptions").map(|o| split_args(&o)).unwrap_or_default();
    let profiles: Vec<String> = conf
        .descendants()
        .find(|n| n.has_tag_name("option") && n.attribute("name") == Some("profilesMap"))
        .map(|o| {
            o.descendants()
                .filter(|n| n.has_tag_name("entry") && n.attribute("value") == Some("true"))
                .filter_map(|n| n.attribute("key").map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if !profiles.is_empty() {
        options.push("-P".into());
        options.push(profiles.join(","));
    }
    let boot = goals.iter().any(|g| g == "spring-boot:run");
    let mut svc = base_service(name, if boot { ServiceKind::MavenBoot } else { ServiceKind::Maven });
    svc.cwd = Some(cwd);
    svc.maven = Some(MavenOpts {
        goals: if boot { Vec::new() } else { goals },
        options,
        ..Default::default()
    });
    svc.env = envs(conf);
    if let Some(jdk) = project.project_jdk.clone() {
        svc.runtime = Some(RuntimeOpts { jdk: Some(jdk), ..Default::default() });
    }
    Converted::Ok(Box::new(candidate(name, svc)))
}

fn gradle(conf: Node, name: &str, project: &Project, ctx: &ScanContext) -> Converted {
    let tasks = deep_list(conf, "taskNames");
    if tasks.is_empty() {
        return Converted::Skip("Gradle 태스크 없음".into());
    }
    let cwd = deep_opt(conf, "externalProjectPath")
        .map(|w| expand_macros(&w, project, &project.root, ctx))
        .unwrap_or_else(|| to_slash(&project.root));
    let options = deep_opt(conf, "scriptParameters").map(|o| split_args(&o)).unwrap_or_default();
    let mut svc = base_service(name, ServiceKind::Gradle);
    if let [only] = tasks.as_slice() {
        if let Some(prefix) = only.strip_suffix("bootRun") {
            svc.kind = ServiceKind::GradleBoot;
            let p = prefix.trim_end_matches(':');
            svc.gradle = Some(GradleOpts { project: (!p.is_empty()).then(|| p.to_string()), options: options.clone(), ..Default::default() });
        }
    }
    if svc.kind == ServiceKind::Gradle {
        svc.gradle = Some(GradleOpts { tasks, options, ..Default::default() });
    }
    svc.cwd = Some(cwd);
    svc.env = envs(conf);
    if let Some(jdk) = project.project_jdk.clone() {
        svc.runtime = Some(RuntimeOpts { jdk: Some(jdk), ..Default::default() });
    }
    Converted::Ok(Box::new(candidate(name, svc)))
}

fn shell(conf: Node, name: &str, project: &Project, ctx: &ScanContext) -> Converted {
    let exec_file = opt(conf, "EXECUTE_SCRIPT_FILE").as_deref() != Some("false");
    let interpreter = opt(conf, "INTERPRETER_PATH").unwrap_or_default().to_lowercase();
    let cwd = opt(conf, "SCRIPT_WORKING_DIRECTORY")
        .or_else(|| opt(conf, "INDEPENDENT_SCRIPT_WORKING_DIRECTORY").filter(|v| v != "true"))
        .map(|w| expand_macros(&w, project, &project.root, ctx))
        .unwrap_or_else(|| to_slash(&project.root));
    let (command, shell) = if exec_file {
        let Some(path) = opt(conf, "SCRIPT_PATH") else {
            return Converted::Skip("스크립트 경로 없음".into());
        };
        let path = expand_macros(&path, project, &project.root, ctx);
        let options = opt(conf, "SCRIPT_OPTIONS").unwrap_or_default();
        let lower = path.to_lowercase();
        if lower.ends_with(".ps1") {
            (format!("& '{path}' {options}").trim().to_string(), ShellKind::Powershell)
        } else if lower.ends_with(".bat") || lower.ends_with(".cmd") {
            (format!("\"{}\" {options}", path.replace('/', "\\")).trim().to_string(), ShellKind::Cmd)
        } else {
            (format!("'{path}' {options}").trim().to_string(), ShellKind::Bash)
        }
    } else {
        let Some(text) = opt(conf, "SCRIPT_TEXT") else {
            return Converted::Skip("스크립트 내용 없음".into());
        };
        let shell = if interpreter.contains("pwsh") {
            ShellKind::Pwsh
        } else if interpreter.contains("powershell") {
            ShellKind::Powershell
        } else if interpreter.contains("cmd") {
            ShellKind::Cmd
        } else {
            ShellKind::Bash
        };
        (text, shell)
    };
    let mut svc = base_service(name, ServiceKind::Shell);
    svc.cwd = Some(cwd);
    svc.shell = Some(ShellOpts { command, shell, ..Default::default() });
    svc.env = envs(conf);
    Converted::Ok(Box::new(candidate(name, svc)))
}
