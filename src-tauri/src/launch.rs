//! 서비스 정의를 실제 실행 명령(LaunchSpec)으로 바꾼다.
//!
//! 프로젝트 소스는 건드리지 않는다. 프로파일·VM 옵션·인자는 모두
//! 명령줄 인자, 환경변수, 대시보드가 만든 Gradle init script로 전달한다.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::{EnvMap, OutputEncoding, PackageManager, Service, ServiceKind, ShellKind, Workspace};
use crate::vars::VarContext;

/// Gradle init script가 읽는 환경변수 (JVM 옵션을 구분자로 이어 붙인 값)
pub const JVM_ARGS_ENV: &str = "SD_JVM_ARGS";
/// JVM 옵션 구분자 (ASCII Unit Separator)
pub const JVM_ARGS_SEP: char = '\u{1F}';

/// `-I`로 넘기는 Gradle init script 내용
pub const GRADLE_INIT_SCRIPT: &str = r#"// Services Dashboard가 생성한 파일입니다. 프로젝트 파일 수정 없이
// JavaExec 태스크(bootRun, run 등)에 JVM 옵션을 주입합니다.
def sdJvmArgs = System.getenv('SD_JVM_ARGS')
if (sdJvmArgs) {
    def parsed = sdJvmArgs.split('\u001F').findAll { it } as List
    allprojects {
        tasks.withType(JavaExec).configureEach {
            jvmArgs(parsed)
        }
    }
}
"#;

#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    /// Windows `cmd /c` 뒤에 그대로 붙일 명령줄 (이스케이프하지 않음)
    pub raw_tail: Option<String>,
    pub cwd: PathBuf,
    /// 대시보드 프로세스 환경 위에 덮어쓸 환경변수
    pub env: EnvMap,
    pub display: String,
    pub encoding: OutputEncoding,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPreview {
    pub command: String,
    pub cwd: String,
    pub env: EnvMap,
}

impl From<&LaunchSpec> for LaunchPreview {
    fn from(s: &LaunchSpec) -> Self {
        LaunchPreview {
            command: s.display.clone(),
            cwd: s.cwd.to_string_lossy().into_owned(),
            env: s.env.clone(),
        }
    }
}

pub struct LaunchContext<'a> {
    pub workspace: &'a Workspace,
    pub workspace_dir: &'a Path,
    pub gradle_init_script: &'a Path,
}

pub fn build(ctx: &LaunchContext, svc: &Service) -> Result<LaunchSpec, String> {
    let vars = VarContext::new(&ctx.workspace.variables, &ctx.workspace_dir.to_string_lossy());
    let cwd = resolve_cwd(&vars, svc, ctx.workspace_dir)?;

    // 환경변수 계층: 워크스페이스 → 그룹 → env 파일 → 서비스 (뒤가 우선)
    let mut env = EnvMap::new();
    for (k, v) in &ctx.workspace.env {
        set_env(&mut env, k, &vars.expand(v)?);
    }
    if let Some(group) = svc.group.as_deref().and_then(|g| ctx.workspace.group(g)) {
        for (k, v) in &group.env {
            set_env(&mut env, k, &vars.expand(v)?);
        }
    }
    for file in &svc.env_files {
        let path = resolve_path(&cwd, &vars.expand(file)?);
        for (k, v) in read_env_file(&path)? {
            set_env(&mut env, &k, &v);
        }
    }
    for (k, v) in &svc.env {
        set_env(&mut env, k, &vars.expand(v)?);
    }

    // 런타임: 이 프로세스에만 JAVA_HOME / PATH를 바꾼다
    let runtime = svc.runtime.as_ref();
    let jdk = vars.expand_opt(runtime.and_then(|r| r.jdk.as_ref()))?;
    let node_dir = vars.expand_opt(runtime.and_then(|r| r.node.as_ref()))?;
    if let Some(jdk) = &jdk {
        let bin = Path::new(jdk).join("bin");
        if !bin.is_dir() {
            return Err(format!("JDK 경로가 올바르지 않습니다 (bin 폴더 없음): {jdk}"));
        }
        set_env(&mut env, "JAVA_HOME", jdk);
        prepend_path(&mut env, &bin.to_string_lossy());
    }
    if let Some(node) = &node_dir {
        if !Path::new(node).is_dir() {
            return Err(format!("Node 경로가 없습니다: {node}"));
        }
        prepend_path(&mut env, node);
    }

    let args = vars.expand_all(&svc.args)?;
    let jvm_args = vars.expand_all(&svc.jvm_args)?;
    let profiles: Vec<String> = svc
        .spring
        .as_ref()
        .map(|s| s.profiles.iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();
    let profile_arg = (!profiles.is_empty()).then(|| format!("--spring.profiles.active={}", profiles.join(",")));

    let mut raw_tail = None;
    let (program, cmd_args): (String, Vec<String>) = match svc.kind {
        ServiceKind::Npm => {
            let npm = svc.npm.clone().unwrap_or_default();
            let script = vars.expand(npm.script.trim())?;
            if script.is_empty() {
                return Err("실행할 npm 스크립트를 입력하세요 (예: dev).".into());
            }
            if !cwd.join("package.json").is_file() {
                return Err(format!("작업 디렉터리에 package.json이 없습니다: {}", cwd.display()));
            }
            let (tool, mut a) = match npm.package_manager {
                PackageManager::Npm => ("npm", vec!["run".to_string(), script]),
                PackageManager::Yarn => ("yarn", vec![script]),
                PackageManager::Pnpm => ("pnpm", vec!["run".to_string(), script]),
            };
            if !args.is_empty() {
                if npm.package_manager == PackageManager::Npm {
                    a.push("--".into());
                }
                a.extend(args);
            }
            (script_tool(node_dir.as_deref(), tool), a)
        }
        ServiceKind::Node => {
            let node = svc.node.clone().unwrap_or_default();
            let entry = vars.expand(node.entry.trim())?;
            if entry.is_empty() {
                return Err("실행할 스크립트 파일을 입력하세요 (예: server.js).".into());
            }
            let mut a = vars.expand_all(&node.node_args)?;
            a.push(entry);
            a.extend(args);
            (exe_tool(node_dir.as_deref(), "node"), a)
        }
        ServiceKind::GradleBoot | ServiceKind::Gradle => {
            let g = svc.gradle.clone().unwrap_or_default();
            let project = vars.expand_opt(g.project.as_ref())?;
            let mut a = vec!["--console=plain".to_string()];
            a.extend(vars.expand_all(&g.options)?);
            if !jvm_args.is_empty() {
                a.push("-I".into());
                a.push(ctx.gradle_init_script.to_string_lossy().into_owned());
                set_env(&mut env, JVM_ARGS_ENV, &jvm_args.join(&JVM_ARGS_SEP.to_string()));
            }
            let tasks: Vec<String> = if svc.kind == ServiceKind::GradleBoot {
                vec!["bootRun".into()]
            } else {
                let t = vars.expand_all(&g.tasks)?;
                if t.iter().all(|t| t.trim().is_empty()) {
                    return Err("실행할 Gradle 태스크를 입력하세요 (예: run).".into());
                }
                t
            };
            for task in tasks.iter().filter(|t| !t.trim().is_empty()) {
                a.push(qualify_gradle_task(project.as_deref(), task.trim()));
            }
            let mut program_args: Vec<String> = profile_arg.into_iter().collect();
            program_args.extend(args);
            if !program_args.is_empty() {
                a.push(format!("--args={}", join_quoted(&program_args)));
            }
            (gradle_program(&cwd), a)
        }
        ServiceKind::MavenBoot | ServiceKind::Maven => {
            let m = svc.maven.clone().unwrap_or_default();
            let mut a = vars.expand_all(&m.options)?;
            if let Some(module) = vars.expand_opt(m.module.as_ref())? {
                a.push("-pl".into());
                a.push(module);
            }
            if svc.kind == ServiceKind::MavenBoot {
                a.push("spring-boot:run".into());
                if !profiles.is_empty() {
                    a.push(format!("-Dspring-boot.run.profiles={}", profiles.join(",")));
                }
                if !jvm_args.is_empty() {
                    a.push(format!("-Dspring-boot.run.jvmArguments={}", join_quoted(&jvm_args)));
                }
                if !args.is_empty() {
                    a.push(format!("-Dspring-boot.run.arguments={}", join_quoted(&args)));
                }
            } else {
                let goals = vars.expand_all(&m.goals)?;
                if goals.iter().all(|g| g.trim().is_empty()) {
                    return Err("실행할 Maven 골을 입력하세요 (예: exec:java).".into());
                }
                a.extend(goals.into_iter().filter(|g| !g.trim().is_empty()));
            }
            (maven_program(&cwd), a)
        }
        ServiceKind::Jar => {
            let jar = svc.jar.clone().unwrap_or_default();
            let path = vars.expand(jar.path.trim())?;
            if path.is_empty() {
                return Err("실행할 jar 파일 경로를 입력하세요.".into());
            }
            let path = resolve_path(&cwd, &path);
            if !path.is_file() {
                return Err(format!("jar 파일이 없습니다: {}", path.display()));
            }
            let mut a = jvm_args;
            a.push("-jar".into());
            a.push(path.to_string_lossy().into_owned());
            a.extend(profile_arg);
            a.extend(args);
            (java_program(&env), a)
        }
        ServiceKind::Shell => {
            let shell = svc.shell.clone().unwrap_or_default();
            let command = vars.expand(shell.command.trim())?;
            if command.is_empty() {
                return Err("실행할 명령을 입력하세요.".into());
            }
            match shell.shell {
                ShellKind::Cmd if cfg!(windows) => {
                    raw_tail = Some(format!("\"{command}\""));
                    ("cmd.exe".into(), vec!["/d".into(), "/s".into(), "/c".into()])
                }
                ShellKind::Cmd => ("sh".into(), vec!["-c".into(), command]),
                ShellKind::Powershell | ShellKind::Pwsh => {
                    let exe = if shell.shell == ShellKind::Pwsh { "pwsh" } else { "powershell" };
                    let exe = if cfg!(windows) { format!("{exe}.exe") } else { exe.to_string() };
                    let a = ["-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]
                        .iter()
                        .map(|s| s.to_string())
                        .chain(std::iter::once(command))
                        .collect();
                    (exe, a)
                }
                ShellKind::Bash => ("bash".into(), vec!["-lc".into(), command]),
            }
        }
        ServiceKind::JavaMain => {
            return Err(
                "java-main 타입(메인 클래스 직접 실행)은 M4에서 지원할 예정입니다. 지금은 gradle-boot 또는 maven-boot를 사용하세요."
                    .into(),
            );
        }
    };

    let mut display = std::iter::once(display_program(&program))
        .chain(cmd_args.iter().map(|a| quote_display(a)))
        .collect::<Vec<_>>()
        .join(" ");
    if let Some(tail) = &raw_tail {
        display.push(' ');
        display.push_str(tail);
    }

    Ok(LaunchSpec {
        program,
        args: cmd_args,
        raw_tail,
        cwd,
        env,
        display,
        encoding: svc.encoding,
    })
}

fn resolve_cwd(vars: &VarContext, svc: &Service, workspace_dir: &Path) -> Result<PathBuf, String> {
    let cwd = match vars.expand_opt(svc.cwd.as_ref())? {
        Some(c) => resolve_path(workspace_dir, &c),
        None if svc.kind == ServiceKind::Shell => dirs::home_dir().unwrap_or_else(|| workspace_dir.to_path_buf()),
        None => return Err("작업 디렉터리를 지정하세요.".into()),
    };
    if !cwd.is_dir() {
        return Err(format!("작업 디렉터리가 없습니다: {}", cwd.display()));
    }
    Ok(cwd)
}

fn resolve_path(base: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() { path } else { base.join(path) }
}

/// Windows에서는 환경변수 이름의 대소문자를 구분하지 않는다.
fn set_env(env: &mut EnvMap, key: &str, value: &str) {
    if cfg!(windows) {
        env.retain(|k, _| !k.eq_ignore_ascii_case(key));
    }
    env.insert(key.to_string(), value.to_string());
}

fn get_env(env: &EnvMap, key: &str) -> Option<String> {
    let found = if cfg!(windows) {
        env.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.clone())
    } else {
        env.get(key).cloned()
    };
    found.or_else(|| std::env::var(key).ok())
}

fn prepend_path(env: &mut EnvMap, dir: &str) {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let new = match get_env(env, "PATH") {
        Some(base) if !base.is_empty() => format!("{dir}{sep}{base}"),
        _ => dir.to_string(),
    };
    set_env(env, "PATH", &new);
}

/// 간단한 .env 파서: `KEY=VALUE`, `#` 주석, `export ` 접두사, 따옴표 제거
fn read_env_file(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("env 파일을 읽을 수 없습니다: {} ({e})", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        let v = if v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) {
            &v[1..v.len() - 1]
        } else {
            v
        };
        out.push((k.trim().to_string(), v.to_string()));
    }
    Ok(out)
}

fn find_upward(start: &Path, name: &str, max_levels: usize) -> Option<PathBuf> {
    let mut dir = Some(start);
    for _ in 0..=max_levels {
        let d = dir?;
        let candidate = d.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

fn gradle_program(cwd: &Path) -> String {
    let wrapper = if cfg!(windows) { "gradlew.bat" } else { "gradlew" };
    match find_upward(cwd, wrapper, 4) {
        Some(p) => p.to_string_lossy().into_owned(),
        None => if cfg!(windows) { "gradle.bat".into() } else { "gradle".into() },
    }
}

fn maven_program(cwd: &Path) -> String {
    let wrapper = if cfg!(windows) { "mvnw.cmd" } else { "mvnw" };
    match find_upward(cwd, wrapper, 4) {
        Some(p) => p.to_string_lossy().into_owned(),
        None => if cfg!(windows) { "mvn.cmd".into() } else { "mvn".into() },
    }
}

fn java_program(env: &EnvMap) -> String {
    if let Some(home) = get_env(env, "JAVA_HOME") {
        let exe = Path::new(&home).join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
        if exe.is_file() {
            return exe.to_string_lossy().into_owned();
        }
    }
    "java".into()
}

/// npm / yarn / pnpm처럼 Windows에서 `.cmd`로 배포되는 도구
fn script_tool(dir: Option<&str>, name: &str) -> String {
    let file = if cfg!(windows) { format!("{name}.cmd") } else { name.to_string() };
    if let Some(dir) = dir {
        let p = Path::new(dir).join(&file);
        if p.is_file() {
            return p.to_string_lossy().into_owned();
        }
    }
    file
}

fn exe_tool(dir: Option<&str>, name: &str) -> String {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    if let Some(dir) = dir {
        let p = Path::new(dir).join(&file);
        if p.is_file() {
            return p.to_string_lossy().into_owned();
        }
    }
    file
}

fn qualify_gradle_task(project: Option<&str>, task: &str) -> String {
    match project.map(|p| p.trim().trim_end_matches(':')) {
        Some(p) if !p.is_empty() && !task.starts_with(':') => {
            let p = if p.starts_with(':') { p.to_string() } else { format!(":{p}") };
            format!("{p}:{task}")
        }
        _ => task.to_string(),
    }
}

/// Gradle `--args`, Maven `jvmArguments`처럼 도구가 다시 공백으로 나누는 값을 만든다.
pub fn join_quoted(items: &[String]) -> String {
    items
        .iter()
        .map(|a| {
            let needs = a.is_empty() || a.chars().any(|c| c.is_whitespace() || c == '"' || c == '\'');
            if !needs {
                a.clone()
            } else if !a.contains('"') {
                format!("\"{a}\"")
            } else {
                format!("'{a}'")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_program(program: &str) -> String {
    // 긴 절대 경로 대신 파일 이름만 보여 준다 (gradlew.bat, java.exe 등)
    let name = Path::new(program)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| program.to_string());
    quote_display(&name)
}

fn quote_display(arg: &str) -> String {
    if arg.is_empty() || arg.chars().any(|c| c.is_whitespace() || c == '"') {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::fs;

    struct Fixture {
        dir: tempfile::TempDir,
        ws: Workspace,
        init: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let mut ws = Workspace::empty("t");
            ws.variables.insert("ROOT".into(), dir.path().to_string_lossy().into_owned());
            ws.env.insert("COMMON".into(), "1".into());
            ws.groups.push(Group {
                id: "server".into(),
                name: "server".into(),
                env: [("SPRING_PROFILES_ACTIVE".to_string(), "group".to_string())].into_iter().collect(),
                ..Default::default()
            });
            let init = dir.path().join("init.gradle");
            Fixture { dir, ws, init }
        }
        fn path(&self) -> &Path {
            self.dir.path()
        }
        fn build(&self, svc: &Service) -> Result<LaunchSpec, String> {
            build(
                &LaunchContext { workspace: &self.ws, workspace_dir: self.path(), gradle_init_script: &self.init },
                svc,
            )
        }
    }

    fn svc(kind: ServiceKind) -> Service {
        Service { id: "s".into(), name: "s".into(), kind, cwd: Some("${var:ROOT}".into()), ..Default::default() }
    }

    #[test]
    fn npm_run_with_args() {
        let f = Fixture::new();
        fs::write(f.path().join("package.json"), "{}").unwrap();
        let mut s = svc(ServiceKind::Npm);
        s.npm = Some(NpmOpts { script: "dev".into(), ..Default::default() });
        s.args = vec!["--port".into(), "5174".into()];
        let spec = f.build(&s).unwrap();
        assert!(spec.program.starts_with("npm"));
        assert_eq!(spec.args, vec!["run", "dev", "--", "--port", "5174"]);
        assert_eq!(spec.env.get("COMMON").unwrap(), "1");
    }

    #[test]
    fn npm_requires_package_json() {
        let f = Fixture::new();
        let mut s = svc(ServiceKind::Npm);
        s.npm = Some(NpmOpts { script: "dev".into(), ..Default::default() });
        assert!(f.build(&s).unwrap_err().contains("package.json"));
    }

    #[test]
    fn gradle_boot_uses_wrapper_init_script_and_args() {
        let f = Fixture::new();
        let wrapper = if cfg!(windows) { "gradlew.bat" } else { "gradlew" };
        fs::write(f.path().join(wrapper), "").unwrap();
        let mut s = svc(ServiceKind::GradleBoot);
        s.group = Some("server".into());
        s.gradle = Some(GradleOpts { project: Some(":apps:portal".into()), ..Default::default() });
        s.spring = Some(SpringOpts { profiles: vec!["local".into(), "db".into()], ..Default::default() });
        s.jvm_args = vec!["--add-opens".into(), "java.base/java.lang=ALL-UNNAMED".into(), "-Dfile.encoding=UTF-8".into()];
        s.args = vec!["--server.port=8081".into(), "--app.title=My App".into()];
        let spec = f.build(&s).unwrap();
        assert!(spec.program.ends_with(wrapper));
        assert!(spec.args.contains(&":apps:portal:bootRun".to_string()));
        assert!(spec.args.contains(&"-I".to_string()));
        assert_eq!(
            spec.args.last().unwrap(),
            "--args=--spring.profiles.active=local,db --server.port=8081 \"--app.title=My App\""
        );
        let jvm = spec.env.get(JVM_ARGS_ENV).unwrap();
        assert_eq!(jvm.split(JVM_ARGS_SEP).count(), 3);
        // 그룹 환경변수가 적용된다
        assert_eq!(spec.env.get("SPRING_PROFILES_ACTIVE").unwrap(), "group");
    }

    #[test]
    fn gradle_boot_without_jvm_args_skips_init_script() {
        let f = Fixture::new();
        let mut s = svc(ServiceKind::GradleBoot);
        s.gradle = Some(GradleOpts::default());
        let spec = f.build(&s).unwrap();
        assert!(!spec.args.contains(&"-I".to_string()));
        assert_eq!(spec.args.last().unwrap(), "bootRun");
        assert!(!spec.env.contains_key(JVM_ARGS_ENV));
    }

    #[test]
    fn maven_boot_properties() {
        let f = Fixture::new();
        let mut s = svc(ServiceKind::MavenBoot);
        s.maven = Some(MavenOpts { module: Some("portal".into()), ..Default::default() });
        s.spring = Some(SpringOpts { profiles: vec!["local".into()], ..Default::default() });
        s.jvm_args = vec!["-Xmx512m".into()];
        s.args = vec!["--server.port=8080".into()];
        let spec = f.build(&s).unwrap();
        assert_eq!(
            spec.args,
            vec![
                "-pl",
                "portal",
                "spring-boot:run",
                "-Dspring-boot.run.profiles=local",
                "-Dspring-boot.run.jvmArguments=-Xmx512m",
                "-Dspring-boot.run.arguments=--server.port=8080",
            ]
        );
    }

    #[test]
    fn jar_with_jdk_runtime() {
        let f = Fixture::new();
        let jdk = f.path().join("jdk");
        fs::create_dir_all(jdk.join("bin")).unwrap();
        let java = jdk.join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
        fs::write(&java, "").unwrap();
        fs::write(f.path().join("app.jar"), "").unwrap();
        let mut s = svc(ServiceKind::Jar);
        s.jar = Some(JarOpts { path: "app.jar".into(), ..Default::default() });
        s.runtime = Some(RuntimeOpts { jdk: Some(jdk.to_string_lossy().into_owned()), ..Default::default() });
        s.jvm_args = vec!["-Xmx256m".into()];
        let spec = f.build(&s).unwrap();
        assert_eq!(PathBuf::from(&spec.program), java);
        assert_eq!(spec.args[0], "-Xmx256m");
        assert_eq!(spec.args[1], "-jar");
        assert_eq!(spec.env.get("JAVA_HOME").unwrap(), &jdk.to_string_lossy().into_owned());
        assert!(spec.env.get("PATH").unwrap().starts_with(&*jdk.join("bin").to_string_lossy()));
    }

    #[test]
    fn invalid_jdk_is_reported() {
        let f = Fixture::new();
        let mut s = svc(ServiceKind::GradleBoot);
        s.runtime = Some(RuntimeOpts { jdk: Some("C:/definitely/not/a/jdk".into()), ..Default::default() });
        assert!(f.build(&s).unwrap_err().contains("JDK"));
    }

    #[test]
    fn env_file_layer_between_group_and_service() {
        let f = Fixture::new();
        fs::write(f.path().join(".env"), "# comment\nexport A=from-file\nB=\"quoted value\"\nC=file\n").unwrap();
        let mut s = svc(ServiceKind::Shell);
        s.shell = Some(ShellOpts { command: "echo hi".into(), ..Default::default() });
        s.env_files = vec![".env".into()];
        s.env.insert("C".into(), "service".into());
        let spec = f.build(&s).unwrap();
        assert_eq!(spec.env.get("A").unwrap(), "from-file");
        assert_eq!(spec.env.get("B").unwrap(), "quoted value");
        assert_eq!(spec.env.get("C").unwrap(), "service");
    }

    #[test]
    fn shell_cmd_uses_raw_tail_on_windows() {
        let f = Fixture::new();
        let mut s = svc(ServiceKind::Shell);
        s.shell = Some(ShellOpts { command: "echo \"a b\" && exit 3".into(), ..Default::default() });
        let spec = f.build(&s).unwrap();
        if cfg!(windows) {
            assert_eq!(spec.program, "cmd.exe");
            assert_eq!(spec.raw_tail.as_deref(), Some("\"echo \"a b\" && exit 3\""));
            assert!(spec.display.ends_with("/c \"echo \"a b\" && exit 3\""));
        }
    }

    #[test]
    fn missing_cwd_is_error() {
        let f = Fixture::new();
        let mut s = svc(ServiceKind::GradleBoot);
        s.cwd = Some("${var:ROOT}/nope".into());
        assert!(f.build(&s).unwrap_err().contains("작업 디렉터리"));
        s.cwd = None;
        assert!(f.build(&s).unwrap_err().contains("작업 디렉터리"));
    }

    #[test]
    fn gradle_task_qualification() {
        assert_eq!(qualify_gradle_task(Some(":apps:x"), "bootRun"), ":apps:x:bootRun");
        assert_eq!(qualify_gradle_task(Some("apps:x:"), "run"), ":apps:x:run");
        assert_eq!(qualify_gradle_task(Some(":"), "bootRun"), "bootRun");
        assert_eq!(qualify_gradle_task(None, "build"), "build");
        assert_eq!(qualify_gradle_task(Some(":a"), ":b:run"), ":b:run");
    }
}
