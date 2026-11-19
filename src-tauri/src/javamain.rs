//! java-main: 메인 클래스를 빌드 도구 없이 직접 실행한다 (IntelliJ의 Application 실행과 같은 방식).
//!
//! 클래스패스는 Gradle/Maven으로 한 번 계산해 캐시하고, 소스·빌드 파일이 바뀌지 않았으면
//! 다음 실행부터는 빌드 도구를 거치지 않아 바로 뜬다.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::model::{ClasspathSource, GradleOpts, MavenOpts, RebuildPolicy, Service, ServiceKind};

/// 클래스패스를 파일로 쓰는 Gradle init script (프로젝트 파일은 건드리지 않는다)
pub const GRADLE_CLASSPATH_SCRIPT: &str = r#"// Services Dashboard가 생성한 파일입니다.
// sdWriteClasspath: 컴파일(classes) 후 main 소스셋의 runtimeClasspath를 SD_CLASSPATH_FILE에 쓴다.
allprojects {
    plugins.withType(JavaPlugin).configureEach {
        def cp = project.sourceSets.main.runtimeClasspath
        tasks.register('sdWriteClasspath') {
            dependsOn 'classes'
            doLast {
                def out = new File(System.getenv('SD_CLASSPATH_FILE'))
                out.parentFile.mkdirs()
                out.text = cp.files.collect { it.absolutePath }.join(File.pathSeparator)
            }
        }
    }
}
"#;

/// 캐시 파일 (`<설정 폴더>/classpath/<서비스 ID>.cp`, `.argfile`)
pub struct CacheFiles {
    pub classpath: PathBuf,
    pub argfile: PathBuf,
    /// Maven dependency:build-classpath 출력 (의존 jar만)
    pub maven_deps: PathBuf,
}

pub fn cache_files(dir: &Path, id: &str) -> CacheFiles {
    let safe: String = id.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    CacheFiles {
        classpath: dir.join(format!("{safe}.cp")),
        argfile: dir.join(format!("{safe}.argfile")),
        maven_deps: dir.join(format!("{safe}.mvn-deps")),
    }
}

const WATCHED_EXT: &[&str] = &["java", "kt", "groovy", "gradle", "kts", "xml", "properties", "yml", "yaml", "toml"];
const SKIP_DIRS: &[&str] = &["node_modules", ".git", ".idea", ".gradle", "build", "target", "out", "bin", "dist", ".vscode", ".run"];

/// root 아래에 since보다 새로 바뀐 소스·빌드 파일이 있는지 (찾으면 바로 멈춘다)
pub fn changed_since(root: &Path, since: SystemTime) -> bool {
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if depth < 25 && !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                    stack.push((e.path(), depth + 1));
                }
            } else if WATCHED_EXT.iter().any(|x| name.rsplit('.').next() == Some(*x))
                && e.metadata().and_then(|m| m.modified()).is_ok_and(|m| m > since)
            {
                return true;
            }
        }
    }
    false
}

/// 이번 실행 전에 빌드(클래스패스 다시 계산)가 필요한지
pub fn needs_build(policy: RebuildPolicy, cache: &Path, root: &Path) -> bool {
    let cached = std::fs::metadata(cache).and_then(|m| m.modified()).ok();
    match (policy, cached) {
        (_, None) => true,
        (RebuildPolicy::Always, _) => true,
        (RebuildPolicy::Never, Some(_)) => false,
        (RebuildPolicy::Auto, Some(t)) => changed_since(root, t),
    }
}

/// 준비 단계를 실행할 서비스 정의 (Gradle/Maven 타입으로 바꿔 launch::build에 넘긴다)
pub fn prepare_service(svc: &Service, files: &CacheFiles, gradle_script: &Path) -> Option<Service> {
    let jm = svc.java_main.clone().unwrap_or_default();
    let mut prep = svc.clone();
    prep.args.clear();
    prep.readiness = None;
    prep.debug = None;
    prep.spring = None;
    match jm.source {
        ClasspathSource::Manual => None,
        ClasspathSource::Gradle => {
            let module = jm.module.as_deref().map(str::trim).filter(|m| !m.is_empty() && *m != ":");
            let task = match module {
                Some(m) => format!("{}:sdWriteClasspath", if m.starts_with(':') { m.to_string() } else { format!(":{m}") }),
                None => "sdWriteClasspath".into(),
            };
            prep.kind = ServiceKind::Gradle;
            prep.gradle = Some(GradleOpts {
                tasks: vec![task],
                options: vec!["-I".into(), gradle_script.to_string_lossy().into_owned()],
                ..Default::default()
            });
            prep.env.insert("SD_CLASSPATH_FILE".into(), files.classpath.to_string_lossy().into_owned());
            Some(prep)
        }
        ClasspathSource::Maven => {
            prep.kind = ServiceKind::Maven;
            prep.maven = Some(MavenOpts {
                module: jm.module.clone().filter(|m| !m.trim().is_empty()),
                goals: vec!["compile".into(), "dependency:build-classpath".into()],
                options: vec![format!("-Dmdep.outputFile={}", files.maven_deps.to_string_lossy()), "-q".into()],
                ..Default::default()
            });
            Some(prep)
        }
    }
}

/// Maven 결과(의존 jar 목록) 앞에 모듈의 target/classes를 붙여 클래스패스 파일을 만든다.
pub fn finish_maven(files: &CacheFiles, module_dir: &Path) -> Result<(), String> {
    let deps = std::fs::read_to_string(&files.maven_deps).map_err(|e| format!("Maven 클래스패스 결과를 읽을 수 없습니다: {e}"))?;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classes = module_dir.join("target").join("classes");
    let joined = format!("{}{sep}{}", classes.to_string_lossy(), deps.trim());
    std::fs::write(&files.classpath, joined).map_err(|e| e.to_string())
}

pub fn read_classpath(file: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("클래스패스 파일을 읽을 수 없습니다: {} ({e})", file.display()))?;
    let sep = if cfg!(windows) { ';' } else { ':' };
    Ok(text.trim().split(sep).map(|s| s.trim().replace('\\', "/")).filter(|s| !s.is_empty()).collect())
}

/// `java @argfile`용 파일: 경로는 슬래시로 바꿔 따옴표 안 이스케이프 문제를 피한다.
pub fn write_argfile(path: &Path, entries: &[String]) -> Result<(), String> {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let cp = entries.iter().map(|e| e.replace('\\', "/")).collect::<Vec<_>>().join(sep);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, format!("-cp\n\"{cp}\"\n")).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::JavaMainOpts;
    use std::time::Duration;

    #[test]
    fn detects_changes_after_cache() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("apps/api/src/main/java/A.java");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, "class A {}").unwrap();
        std::fs::create_dir_all(dir.path().join("build")).unwrap();
        let cache = dir.path().join("cache.cp");
        assert!(needs_build(RebuildPolicy::Auto, &cache, dir.path()), "캐시가 없으면 빌드");
        std::fs::write(&cache, "x").unwrap();
        // 캐시보다 오래된 소스만 있으면 빌드 생략
        let old = SystemTime::now() - Duration::from_secs(60);
        std::fs::File::options().write(true).open(&src).unwrap().set_modified(old).unwrap();
        assert!(!needs_build(RebuildPolicy::Auto, &cache, dir.path()));
        assert!(needs_build(RebuildPolicy::Always, &cache, dir.path()));
        // 빌드 산출물(build/)이 바뀐 것은 무시
        std::fs::write(dir.path().join("build/out.class"), "x").unwrap();
        assert!(!needs_build(RebuildPolicy::Auto, &cache, dir.path()));
        // 소스가 바뀌면 빌드
        std::fs::File::options().write(true).open(&src).unwrap().set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        assert!(needs_build(RebuildPolicy::Auto, &cache, dir.path()));
        assert!(!needs_build(RebuildPolicy::Never, &cache, dir.path()));
    }

    #[test]
    fn prepare_specs() {
        let files = cache_files(Path::new("C:/cfg/classpath"), "api/1");
        assert!(files.classpath.ends_with("api_1.cp"));
        let mut svc = Service { id: "api".into(), kind: ServiceKind::JavaMain, args: vec!["--x".into()], ..Default::default() };
        svc.java_main = Some(JavaMainOpts { module: Some("apps:api".into()), ..Default::default() });
        let prep = prepare_service(&svc, &files, Path::new("C:/cfg/gradle/sd-classpath.gradle")).unwrap();
        assert_eq!(prep.kind, ServiceKind::Gradle);
        assert_eq!(prep.gradle.as_ref().unwrap().tasks, vec![":apps:api:sdWriteClasspath"]);
        assert!(prep.args.is_empty());
        assert!(prep.env.contains_key("SD_CLASSPATH_FILE"));
        svc.java_main = Some(JavaMainOpts { source: ClasspathSource::Maven, module: Some("api".into()), ..Default::default() });
        let prep = prepare_service(&svc, &files, Path::new("x")).unwrap();
        assert_eq!(prep.maven.as_ref().unwrap().goals, vec!["compile", "dependency:build-classpath"]);
        svc.java_main = Some(JavaMainOpts { source: ClasspathSource::Manual, ..Default::default() });
        assert!(prepare_service(&svc, &files, Path::new("x")).is_none());
    }

    #[test]
    fn classpath_files() {
        let dir = tempfile::tempdir().unwrap();
        let files = cache_files(dir.path(), "m");
        std::fs::write(&files.maven_deps, if cfg!(windows) { "C:\\m2\\a.jar;C:\\m2\\b.jar\n" } else { "/m2/a.jar:/m2/b.jar\n" }).unwrap();
        finish_maven(&files, Path::new("proj/api")).unwrap();
        let cp = read_classpath(&files.classpath).unwrap();
        assert_eq!(cp.len(), 3);
        assert!(cp[0].ends_with("proj/api/target/classes"));
        write_argfile(&files.argfile, &cp).unwrap();
        let text = std::fs::read_to_string(&files.argfile).unwrap();
        assert!(text.starts_with("-cp\n\"") && !text.contains('\\'));
    }
}
