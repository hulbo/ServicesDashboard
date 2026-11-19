//! 가상 프로젝트로 가져오기 전체 흐름을 확인한다.

use std::fs;
use std::path::{Path, PathBuf};

use super::project::test_support::{applies_boot_plugin, parse_settings};
use super::*;
use crate::model::{PackageManager, ServiceKind, ShellKind};
use crate::runtime::{RuntimeInfo, RuntimeKind};

fn write(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    jdk17: PathBuf,
    jetbrains: Vec<PathBuf>,
    runtimes: Vec<RuntimeInfo>,
    init: PathBuf,
}

impl Fixture {
    fn ctx(&self) -> ScanContext<'_> {
        ScanContext { runtimes: &self.runtimes, jetbrains_dirs: &self.jetbrains, user_home: None, gradle_init_script: &self.init }
    }
    fn scan(&self) -> ScanResult {
        scan(&self.root, &self.ctx()).unwrap()
    }
}

/// Gradle 멀티 프로젝트 + IntelliJ 구성 + 프론트엔드 2개
fn gradle_fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_path_buf();
    let root = base.join("shop");
    let r = root.as_path();

    write(r, "settings.gradle", "rootProject.name = \"shop\"\ninclude(\"libs:core\")\ninclude 'apps:api',\n        'apps:admin'\ninclude(\"front\")\n");
    write(
        r,
        "build.gradle",
        "plugins {\n    id \"org.springframework.boot\" version \"3.3.0\" apply false\n}\nconfigure(subprojects.findAll { it.hasProperty('app') }) {\n    apply plugin: 'org.springframework.boot'\n}\n",
    );
    write(r, "gradlew.bat", "");
    write(r, "apps/api/build.gradle", "dependencies {}\n");
    write(r, "apps/api/src/main/java/com/example/api/ApiApplication.java", "package com.example.api;\n\n@SpringBootApplication\npublic class ApiApplication {}\n");
    write(r, "apps/api/src/main/resources/application-local.yml", "");
    write(r, "apps/api/src/main/resources/application-prod.yml", "");
    write(r, "apps/api/src/main/resources/application.yml", "spring:\n  config:\n    activate:\n      on-profile: qa\n");
    write(r, "apps/admin/build.gradle", "");
    write(r, "apps/admin/src/main/java/com/example/admin/AdminApplication.java", "package com.example.admin;\n@SpringBootApplication\nclass AdminApplication {}\n");
    write(r, "libs/core/build.gradle", "");
    write(r, "libs/core/src/main/java/com/example/core/Util.java", "package com.example.core;\nclass Util {}\n");
    write(r, "front/portal/package.json", r#"{"name":"@shop/portal","scripts":{"build":"vite build","dev":"vite"}}"#);
    write(r, "front/portal/node_modules/x/package.json", r#"{"scripts":{"dev":"x"}}"#);
    write(r, "front/admin/package.json", r#"{"name":"admin-ui","scripts":{"serve":"vue-cli-service serve","lint":"eslint"}}"#);
    write(r, "front/admin/yarn.lock", "");
    write(r, "tools/package.json", r#"{"name":"tools","scripts":{"gen":"node gen.js"}}"#);

    // IntelliJ: 프로젝트 SDK 이름은 jdk.table.xml에서 경로로 바뀐다
    let jdk17 = base.join("jdks/jdk-17.0.15");
    fs::create_dir_all(jdk17.join("bin")).unwrap();
    let ij = base.join("JetBrains/IntelliJIdea2024.1");
    write(
        &ij,
        "options/jdk.table.xml",
        &format!(
            r#"<application><component name="ProjectJdkTable"><jdk version="2"><name value="corretto-17" /><type value="JavaSDK" /><homePath value="{}" /></jdk></component></application>"#,
            util::to_slash(&jdk17)
        ),
    );
    write(r, ".idea/misc.xml", r#"<project version="4"><component name="ProjectRootManager" version="2" project-jdk-name="corretto-17" project-jdk-type="JavaSDK" /></project>"#);

    write(
        r,
        ".run/ApiApplication.run.xml",
        r#"<component name="ProjectRunConfigurationManager">
  <configuration default="false" name="ApiApplication" type="Application" factoryName="Application" nameIsGenerated="true">
    <option name="MAIN_CLASS_NAME" value="com.example.api.ApiApplication" />
    <module name="shop.apps.api.main" />
    <option name="VM_PARAMETERS" value="--add-opens java.base/java.lang=ALL-UNNAMED -Dfile.encoding=UTF-8" />
    <option name="WORKING_DIRECTORY" value="$MODULE_WORKING_DIR$" />
    <method v="2"><option name="Make" enabled="true" /></method>
  </configuration>
</component>"#,
    );
    write(
        r,
        ".run/api-qa.run.xml",
        r#"<component name="ProjectRunConfigurationManager">
  <configuration default="false" name="api-qa" type="Application" factoryName="Application">
    <option name="MAIN_CLASS_NAME" value="com.example.api.ApiApplication" />
    <module name="shop.apps.api.main" />
    <option name="PROGRAM_PARAMETERS" value="--spring.profiles.active=qa --server.port=8091" />
    <envs><env name="TZ" value="Asia/Seoul" /></envs>
  </configuration>
</component>"#,
    );
    write(
        r,
        ".run/npm-portal.run.xml",
        r#"<component name="ProjectRunConfigurationManager">
  <configuration default="false" name="portal" type="js.build_tools.npm" folderName="npm" nameIsGenerated="true">
    <package-json value="$PROJECT_DIR$/front/portal/package.json" />
    <command value="run" />
    <scripts><script value="dev" /></scripts>
    <arguments value="--port 5174" />
    <node-interpreter value="project" />
    <package-manager value="npm" />
    <envs><env name="VITE_API" value="http://localhost:8080" /></envs>
    <method v="2" />
  </configuration>
</component>"#,
    );
    write(
        r,
        ".run/tools.run.xml",
        r#"<component name="ProjectRunConfigurationManager">
  <configuration default="false" name="jar-app" type="JarApplication">
    <option name="JAR_PATH" value="$PROJECT_DIR$/dist/app.jar" />
    <option name="VM_PARAMETERS" value="-Xmx256m" />
    <option name="ALTERNATIVE_JRE_PATH_ENABLED" value="true" />
    <option name="ALTERNATIVE_JRE_PATH" value="corretto-17" />
  </configuration>
  <configuration default="false" name="redis" type="ShConfigurationType">
    <option name="SCRIPT_TEXT" value="docker compose up redis" />
    <option name="INDEPENDENT_SCRIPT_PATH" value="true" />
    <option name="SCRIPT_PATH" value="" />
    <option name="EXECUTE_SCRIPT_FILE" value="false" />
    <option name="INTERPRETER_PATH" value="C:/Windows/System32/cmd.exe" />
  </configuration>
  <configuration default="false" name="all" type="CompoundRunConfigurationType">
    <toRun name="portal" type="js.build_tools.npm" />
  </configuration>
  <configuration default="false" name="py" type="PythonConfigurationType" />
</component>"#,
    );
    write(
        r,
        ".idea/workspace.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project version="4">
  <component name="PropertiesComponent"><![CDATA[{ "keyToString": { "nodejs_interpreter_path": "NODE_PLACEHOLDER" } }]]></component>
  <component name="RunManager" selected="Spring Boot.ApiApplication">
    <configuration name="ApiApplication" type="SpringBootApplicationConfigurationType" factoryName="Spring Boot" folderName="server" nameIsGenerated="true">
      <module name="shop.apps.api.main" />
      <option name="SPRING_BOOT_MAIN_CLASS" value="com.example.api.ApiApplication" />
      <option name="ACTIVE_PROFILES" value="local" />
      <option name="VM_PARAMETERS" value="-Xmx1g" />
      <additionalParameters>
        <param><option name="enabled" value="true" /><option name="name" value="server.port" /><option name="value" value="8081" /></param>
        <param><option name="enabled" value="false" /><option name="name" value="debug" /><option name="value" value="true" /></param>
      </additionalParameters>
    </configuration>
    <configuration name="Gradle bootRun" type="GradleRunConfiguration" factoryName="Gradle" temporary="true">
      <ExternalSystemSettings>
        <option name="externalProjectPath" value="$PROJECT_DIR$" />
        <option name="scriptParameters" value="--offline" />
        <option name="taskNames"><list><option value=":apps:admin:bootRun" /></list></option>
      </ExternalSystemSettings>
    </configuration>
    <configuration default="true" type="SpringBootApplicationConfigurationType" factoryName="Spring Boot">
      <method v="2" />
    </configuration>
  </component>
</project>"#,
    );
    // Node 인터프리터 경로를 실제 폴더로 바꿔 넣는다
    let node_dir = base.join("nodejs");
    fs::create_dir_all(&node_dir).unwrap();
    fs::write(node_dir.join(if cfg!(windows) { "node.exe" } else { "node" }), "").unwrap();
    let ws = fs::read_to_string(r.join(".idea/workspace.xml")).unwrap();
    let node_exe = util::to_slash(&node_dir.join("node"));
    fs::write(r.join(".idea/workspace.xml"), ws.replace("NODE_PLACEHOLDER", &node_exe)).unwrap();

    let init = base.join("init.gradle");
    Fixture { _dir: dir, root, jdk17, jetbrains: vec![ij], runtimes: Vec::new(), init }
}

fn by_name<'a>(r: &'a ScanResult, name: &str) -> &'a Candidate {
    r.candidates.iter().find(|c| c.service.name == name).unwrap_or_else(|| {
        panic!("후보 '{name}' 없음: {:?}", r.candidates.iter().map(|c| &c.service.name).collect::<Vec<_>>())
    })
}

#[test]
fn intellij_java_configs_become_gradle_boot() {
    let f = gradle_fixture();
    let r = f.scan();
    let jdk = util::to_slash(&f.jdk17);

    // .run의 Application 구성 → 루트가 Boot를 적용하므로 gradle-boot
    let api_runs: Vec<&Candidate> = r.candidates.iter().filter(|c| c.service.name == "ApiApplication").collect();
    assert_eq!(api_runs.len(), 2, ".run과 workspace.xml의 정의가 달라 둘 다 남는다");
    let from_run = api_runs.iter().find(|c| c.origin == ".run/ApiApplication.run.xml").unwrap();
    let s = &from_run.service;
    assert_eq!(s.kind, ServiceKind::GradleBoot);
    assert_eq!(s.gradle.as_ref().unwrap().project.as_deref(), Some(":apps:api"));
    assert_eq!(s.main_class.as_deref(), Some("com.example.api.ApiApplication"));
    assert_eq!(s.jvm_args, vec!["--add-opens", "java.base/java.lang=ALL-UNNAMED", "-Dfile.encoding=UTF-8"]);
    assert_eq!(s.runtime.as_ref().unwrap().jdk.as_deref(), Some(jdk.as_str()), "프로젝트 SDK 이름 → jdk.table 경로");
    assert_eq!(s.cwd.as_deref(), Some(util::to_slash(&f.root).as_str()));
    assert!(from_run.recommended);
    assert!(from_run.command.as_deref().unwrap().contains(":apps:api:bootRun"), "{:?}", from_run.command_error);
    assert_eq!(from_run.key, "intellij:.run/ApiApplication.run.xml#ApiApplication");

    // workspace.xml의 Spring Boot 구성: 프로파일, 구성 속성 재정의, 폴더 → 그룹
    let from_ws = api_runs.iter().find(|c| c.origin == ".idea/workspace.xml").unwrap();
    assert_eq!(from_ws.group.as_deref(), Some("server"));
    assert_eq!(from_ws.service.spring.as_ref().unwrap().profiles, vec!["local"]);
    assert_eq!(from_ws.service.args, vec!["--server.port=8081"]);
    assert_eq!(from_ws.service.ports, vec![8081]);

    // 프로그램 인자와 환경변수
    let qa = by_name(&r, "api-qa");
    assert_eq!(qa.service.args, vec!["--spring.profiles.active=qa", "--server.port=8091"]);
    assert_eq!(qa.service.env.get("TZ").map(String::as_str), Some("Asia/Seoul"));
}

#[test]
fn intellij_other_types() {
    let f = gradle_fixture();
    let r = f.scan();

    let portal = by_name(&r, "portal");
    assert_eq!(portal.source, SourceKind::Intellij);
    assert_eq!(portal.group.as_deref(), Some("npm"));
    let s = &portal.service;
    assert_eq!(s.kind, ServiceKind::Npm);
    assert!(s.cwd.as_deref().unwrap().ends_with("front/portal"));
    assert_eq!(s.npm.as_ref().unwrap().script, "dev");
    assert_eq!(s.args, vec!["--port", "5174"]);
    assert_eq!(s.ports, vec![5174]);
    assert_eq!(s.env.get("VITE_API").map(String::as_str), Some("http://localhost:8080"));
    assert!(s.runtime.as_ref().unwrap().node.as_deref().unwrap().ends_with("/nodejs"), "프로젝트 Node 인터프리터");
    assert_eq!(portal.scripts, vec!["build", "dev"]);

    let jar = by_name(&r, "jar-app");
    assert_eq!(jar.service.kind, ServiceKind::Jar);
    assert!(jar.service.jar.as_ref().unwrap().path.ends_with("/dist/app.jar"));
    assert_eq!(jar.service.runtime.as_ref().unwrap().jdk.as_deref(), Some(util::to_slash(&f.jdk17).as_str()), "대체 JRE 이름");
    assert!(jar.command_error.as_deref().unwrap().contains("jar 파일이 없습니다"), "없는 jar는 미리보기 오류로 보여 준다");

    let redis = by_name(&r, "redis");
    assert_eq!(redis.service.kind, ServiceKind::Shell);
    let sh = redis.service.shell.as_ref().unwrap();
    assert_eq!((sh.command.as_str(), sh.shell), ("docker compose up redis", ShellKind::Cmd));

    let boot_run = by_name(&r, "Gradle bootRun");
    assert_eq!(boot_run.service.kind, ServiceKind::GradleBoot);
    assert_eq!(boot_run.service.gradle.as_ref().unwrap().project.as_deref(), Some(":apps:admin"));
    assert_eq!(boot_run.service.gradle.as_ref().unwrap().options, vec!["--offline"]);
    assert!(!boot_run.recommended, "임시 구성은 기본 선택하지 않는다");

    // 템플릿·복합·미지원 타입은 후보에 없고 경고로 알린다
    assert!(!r.candidates.iter().any(|c| c.service.name == "all" || c.service.name == "py"));
    assert!(r.warnings.iter().any(|w| w.contains("all (복합") && w.contains("py (지원하지 않는 타입")), "{:?}", r.warnings);
    // 모든 후보에 출처 정보가 붙는다
    for c in &r.candidates {
        let src = c.service.extra.get("source").unwrap();
        assert_eq!(src["key"], c.key.as_str());
        assert_eq!(src["root"], util::to_slash(&f.root).as_str());
    }
}

#[test]
fn scanners_fill_gaps_without_duplicates() {
    let f = gradle_fixture();
    let r = f.scan();

    // Spring: api는 IntelliJ 구성이 있어 빠지고, admin은 Gradle bootRun 구성(메인 클래스 없음)이 있어 빠진다.
    let spring: Vec<&Candidate> = r.candidates.iter().filter(|c| c.source == SourceKind::Gradle).collect();
    assert!(spring.is_empty(), "{:?}", spring.iter().map(|c| &c.key).collect::<Vec<_>>());

    // npm: portal은 IntelliJ에 있으므로 빠지고, admin·tools만 남는다 (node_modules 안은 무시)
    let npm: Vec<&Candidate> = r.candidates.iter().filter(|c| c.source == SourceKind::Npm).collect();
    let names: Vec<&str> = npm.iter().map(|c| c.service.name.as_str()).collect();
    assert_eq!(names, vec!["admin-ui", "tools"]);
    let admin = npm[0];
    assert_eq!(admin.service.npm.as_ref().unwrap().script, "serve");
    assert_eq!(admin.service.npm.as_ref().unwrap().package_manager, PackageManager::Yarn);
    // IntelliJ 구성이 있으면 스캐너 후보는 기본 선택하지 않는다
    assert!(npm.iter().all(|c| !c.recommended));
    assert_eq!(r.suggested_var, "SHOP");
}

#[test]
fn spring_scanner_without_intellij() {
    let f = gradle_fixture();
    fs::remove_dir_all(f.root.join(".run")).unwrap();
    fs::remove_dir_all(f.root.join(".idea")).unwrap();
    let r = f.scan();
    let spring: Vec<&Candidate> = r.candidates.iter().filter(|c| c.source == SourceKind::Gradle).collect();
    assert_eq!(spring.len(), 2);
    let api = spring.iter().find(|c| c.service.name == "ApiApplication").unwrap();
    assert_eq!(api.service.gradle.as_ref().unwrap().project.as_deref(), Some(":apps:api"));
    assert_eq!(api.profiles, vec!["local", "prod", "qa"]);
    assert_eq!(api.service.spring.as_ref().unwrap().profiles, vec!["local"], "local 프로파일을 기본 선택");
    assert_eq!(api.group.as_deref(), Some("server"));
    assert!(api.recommended);
    // libs:core는 @SpringBootApplication이 없어 후보가 아니다
    assert!(!spring.iter().any(|c| c.key.contains("libs")));
    // IntelliJ가 없으면 npm 스크립트 후보 중 서비스형만 기본 선택
    let tools = by_name(&r, "tools");
    assert!(!tools.recommended);
    assert!(by_name(&r, "portal").recommended);
}

#[test]
fn maven_project_detection() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path().join("mvn-shop");
    write(&r, "pom.xml", r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><artifactId>shop-parent</artifactId><modules><module>web</module></modules></project>"#);
    write(
        &r,
        "web/pom.xml",
        r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><parent><artifactId>shop-parent</artifactId></parent><artifactId>shop-web</artifactId><build><plugins><plugin><artifactId>spring-boot-maven-plugin</artifactId></plugin></plugins></build></project>"#,
    );
    write(&r, "web/src/main/java/com/example/web/WebApplication.java", "package com.example.web;\n@SpringBootApplication\npublic class WebApplication {}\n");
    write(
        &r,
        ".run/web.run.xml",
        r#"<component name="ProjectRunConfigurationManager"><configuration name="web" type="SpringBootApplicationConfigurationType"><module name="shop-web" /><option name="SPRING_BOOT_MAIN_CLASS" value="com.example.web.WebApplication" /></configuration></component>"#,
    );
    let init = dir.path().join("init.gradle");
    let ctx = ScanContext { runtimes: &[], jetbrains_dirs: &[], user_home: None, gradle_init_script: &init };
    let result = scan(&r, &ctx).unwrap();
    let web = result.candidates.iter().find(|c| c.service.name == "web").unwrap();
    assert_eq!(web.service.kind, ServiceKind::MavenBoot);
    assert_eq!(web.service.maven.as_ref().unwrap().module.as_deref(), Some("web"));
    // 같은 모듈·메인 클래스의 스캐너 후보는 중복이라 빠진다
    assert!(!result.candidates.iter().any(|c| c.source == SourceKind::Maven));
    assert!(web.command.as_deref().unwrap().contains("-pl web spring-boot:run"), "{:?}", web.command_error);
}

#[test]
fn settings_and_plugin_parsing() {
    let (name, projects) = parse_settings("rootProject.name = 'x'\n// include 'old'\ninclude ':a', 'b:c',\n   'd'\ninclude(\"e\")\nincludeBuild('tools')\n");
    assert_eq!(name.as_deref(), Some("x"));
    assert_eq!(projects, vec![":a", ":b:c", ":d", ":e"]);
    assert!(applies_boot_plugin("subprojects {\n  apply plugin: 'org.springframework.boot'\n}"));
    assert!(applies_boot_plugin("plugins { id 'org.springframework.boot' version '3.2.0' }"));
    assert!(!applies_boot_plugin("plugins { id 'org.springframework.boot' version '3.2.0' apply false }"));
    assert!(!applies_boot_plugin("implementation 'org.springframework.boot:spring-boot-starter-web'"));
}

#[test]
fn module_name_mapping() {
    let f = gradle_fixture();
    let mut warnings = Vec::new();
    let p = Project::detect(&f.root, &f.ctx(), &mut warnings);
    assert_eq!(p.gradle_path_for_module("shop.apps.api.main"), ":apps:api");
    assert_eq!(p.gradle_path_for_module("shop.main"), "");
    assert_eq!(p.gradle_path_for_module("shop.libs.core.test"), ":libs:core");
    // 루트 이름이 달라도 include 목록으로 맞춘다
    assert_eq!(p.gradle_path_for_module("other.apps.admin.main"), ":apps:admin");
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn unknown_sdk_warns_once() {
    let f = gradle_fixture();
    write(&f.root, ".idea/misc.xml", r#"<project><component name="ProjectRootManager" project-jdk-name="99" /></project>"#);
    let r = f.scan();
    assert_eq!(r.warnings.iter().filter(|w| w.contains("SDK '99'")).count(), 1);
    assert!(by_name(&r, "api-qa").service.runtime.is_none());
}

#[test]
fn alias_runtime_used_when_table_missing() {
    let mut f = gradle_fixture();
    write(&f.root, ".idea/misc.xml", r#"<project><component name="ProjectRootManager" project-jdk-name="17" /></project>"#);
    f.runtimes = vec![RuntimeInfo {
        kind: RuntimeKind::Jdk,
        path: util::to_slash(&f.jdk17),
        version: "17.0.15".into(),
        major: "17".into(),
        vendor: None,
        source: "test".into(),
    }];
    let r = f.scan();
    assert_eq!(by_name(&r, "api-qa").service.runtime.as_ref().unwrap().jdk.as_deref(), Some(util::to_slash(&f.jdk17).as_str()));
}

/// 실제 프로젝트 스캔 결과를 눈으로 확인할 때 쓴다 (기본 실행에서 제외).
/// `SD_SCAN_ROOT=<경로> cargo test scan_real_project -- --ignored --nocapture`
#[test]
#[ignore]
fn scan_real_project() {
    let Ok(root) = std::env::var("SD_SCAN_ROOT") else { return };
    let env = crate::runtime::DetectEnv::from_system();
    let runtimes = crate::runtime::detect(&env);
    for r in &runtimes {
        println!("런타임 {:?} {} ({}) {}", r.kind, r.version, r.source, r.path);
    }
    let init = std::env::temp_dir().join("sd-init.gradle");
    let ctx = ScanContext { runtimes: &runtimes, jetbrains_dirs: &env.jetbrains_dirs, user_home: env.user_home.clone(), gradle_init_script: &init };
    let t = std::time::Instant::now();
    let r = scan(Path::new(&root), &ctx).unwrap();
    println!("스캔 {:?}, 변수 {}", t.elapsed(), r.suggested_var);
    for w in &r.warnings {
        println!("경고: {w}");
    }
    for c in &r.candidates {
        println!(
            "[{}] {:?} {} | 그룹 {:?} | {} | {}",
            if c.recommended { "v" } else { " " },
            c.source,
            c.service.name,
            c.group,
            c.origin,
            c.command.as_deref().or(c.command_error.as_deref()).unwrap_or("")
        );
        for w in &c.warnings {
            println!("      ⚠ {w}");
        }
    }
}
