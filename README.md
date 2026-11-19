# Services Dashboard

IntelliJ를 켜지 않고 로컬 개발 서비스를 띄우는 **가벼운 데스크톱 서비스 대시보드**.

Node(npm), Spring Boot(Gradle / Maven), jar, 임의 명령으로 된 서비스를 한 화면에서 실행·중지·재시작하고, 서비스마다 콘솔 로그를 본다. IntelliJ의 Services 탭처럼 **왼쪽에는 서비스 트리, 오른쪽에는 콘솔 로그**를 둔다.

> **현재 상태: M3 완료.** 서비스 실행·로그(M1), 설정 가져오기·런타임 감지(M2)에 더해 로그 검색·필터, 준비 감지, 포트 충돌 처리, 트레이 상주·알림, 로그 파일 저장이 된다. 의존 순서 실행·프리셋·자동 재시작은 M4에서 추가한다. 전체 계획은 [docs/PLANNING.md](docs/PLANNING.md)에 있다.

## 왜 만드나

- 서비스 몇 개 띄우려고 무거운 IDE를 켜고 싶지 않다.
- 실행 설정이 `.run/*.run.xml`, `.idea/workspace.xml`, `package.json`, 프로파일, 환경변수, JDK 경로로 흩어져 있다.
- 터미널을 여러 개 띄우면 관리가 어렵고, 창을 닫아도 자식 프로세스가 남아 포트를 붙잡는다.

## 화면

```
+------------------------------+-------------------------------------------------+
| [+] [그룹+] [⤓] [■■] [RT][⚙][WS]| ● PortalApplication  실행 중  PID 1234 · 3분    |
| [ 서비스 검색            ]   |                            [■ 중지] [⟳ 재시작] |
| v server            1/3      |  콘솔 | 설정                                     |
|     ● PortalApi    :8080     | > Task :apps:portal:bootRun                     |
|     ○ AuthApi      :8081     |  :: Spring Boot ::  (v3.3.4)                    |
| v npm               2/4      |  ... Started PortalApplication in 3.4 seconds   |
|     ● portal       :5173     |                                                 |
|     ✕ admin       exit 1     |                                                 |
+------------------------------+-------------------------------------------------+
| ● 실행 3 / 전체 7 · my-project                                         v0.1.0  |
+--------------------------------------------------------------------------------+
```

## 되는 것

| 기능 | 설명 |
|---|---|
| 서비스 트리 | 그룹(폴더)별 목록, 상태 아이콘, 포트·종료 코드 표시, 검색, 드래그 앤 드롭으로 순서·그룹 변경, 우클릭 메뉴, 이름 바꾸기(F2) |
| 실행 타입 | `npm`(npm/yarn/pnpm), `node`, `gradle-boot`, `maven-boot`, `jar`, `gradle`, `maven`, `shell`(cmd/PowerShell/pwsh/bash) |
| 실행 제어 | 시작·중지·재시작, 그룹 전체 시작·중지, 전체 중지 |
| 확실한 종료 | 서비스마다 Windows Job Object를 만들어 프로세스 트리 전체를 종료. 대시보드가 비정상 종료돼도 서비스 프로세스가 고아로 남지 않음 |
| 콘솔 | 서비스별 2만 줄 링 버퍼, 가상 스크롤, ANSI 색상(16/256/트루컬러), stderr 구분, 자동 스크롤과 "새 로그 N줄" 표시, 전체 복사, 지우기(Ctrl+L) |
| **콘솔 검색·필터 (M3)** | 검색(Ctrl+F, 대소문자·정규식, Enter/Shift+Enter로 이동), 일치 줄만 보기, ERROR·WARN 줄 강조와 개수·레벨 필터(스택트레이스 포함), 긴 스택트레이스 접기, 수신 시각 표시, URL 클릭으로 브라우저 열기, 파일로 저장 |
| **준비 감지 (M3)** | 시작하면 '시작 중'으로 두었다가 준비 신호(로그 패턴·포트 열림·HTTP 헬스 URL)가 오면 '실행 중'으로 바꾸고 기동 시간을 보여 줌. 기본: Spring Boot `Started … in N seconds`, npm/node `Local: http://…` 등. 서비스별로 바꾸거나 끌 수 있음 |
| **포트·URL 감지 (M3)** | 로그의 `http://localhost:5173/`, `Tomcat started on port 8080 … context path '/api'` 등에서 주소를 찾아 헤더·트리에 표시하고 클릭하면 브라우저로 염 |
| **포트 충돌 처리 (M3)** | 시작 전에 설정한 포트와 지난번에 감지한 포트를 다른 프로세스가 쓰는지 확인. 점유 프로세스(이름·PID, 또는 대시보드 서비스)를 보여 주고 *종료하고 시작* 제공 |
| **트레이·알림 (M3)** | 창을 닫으면 트레이로 내려가 서비스가 계속 실행됨. 트레이 메뉴에서 그룹별 시작·중지, 전체 중지, 종료. 비정상 종료와 (창을 보고 있지 않을 때) 준비 완료를 Windows 알림으로 알림. 앱을 다시 실행하면 새로 뜨지 않고 기존 창이 나타남 |
| **로그 파일 (M3)** | 설정에서 켜면 `설정 폴더\logs\<서비스 ID>\YYYY-MM-DD.log`에 저장, 보관 기간이 지나면 삭제 |
| 한글 로그 | 출력 인코딩 자동 판별(UTF-8 → 실패 시 CP949), 서비스별 고정 가능 |
| 설정 폼 | 타입별 입력 항목, 환경변수 편집(비밀번호류 값 가림), env 파일, 프로파일, VM 옵션, 인자, 포트, 메모, **실행 명령 미리보기**, 저장(Ctrl+S)·저장 후 재시작 |
| **가져오기 (M2)** | 프로젝트 폴더를 고르면 IntelliJ 실행 구성(`.run/*.run.xml`, `.idea/workspace.xml`)·`package.json` 스크립트·Spring Boot 모듈을 찾아 서비스로 변환. 미리보기에서 고르고 이름·그룹·스크립트·프로파일을 고친 뒤 추가 |
| **다시 가져오기 (M2)** | 원본 구성이 바뀌면 서비스 우클릭 → *원본에서 다시 가져오기*. 실행 정의만 갱신하고 이름·그룹·메모는 유지 |
| **런타임 감지 (M2)** | JDK(JAVA_HOME, Program Files, IntelliJ가 받은 JDK, IntelliJ JDK 목록, PATH), Node(PATH, nvm, Volta, fnm), Maven(MAVEN_HOME, PATH, IntelliJ 번들)을 찾아 목록으로 보여 줌 |
| 런타임 지정 | 서비스별 JDK·Node·Maven을 감지된 목록에서 고르거나 **버전 별칭**(`17`, `1.8`, `20`)으로 지정. 이 프로세스에만 `JAVA_HOME`·`PATH` 적용 |
| **자동 옵션 (M2)** | 한글 깨짐 방지(Java와 Gradle·Maven JVM에 `-Dfile.encoding=UTF-8` 등), 컬러 로그(`FORCE_COLOR=1`, `--spring.output.ansi.enabled=always`, `-Dstyle.color=always`). 서비스별로 끌 수 있고 직접 지정한 값은 그대로 둔다 |
| 소스 수정 없음 | Gradle `bootRun`의 VM 옵션·메인 클래스는 대시보드가 만든 init script(`-I`)로 주입, Maven은 `-Dspring-boot.run.*` 속성 사용 |
| 변수·계층 | `${var:ROOT}` 워크스페이스 변수, `${env:NAME}`, 환경변수 계층(워크스페이스 → 그룹 → env 파일 → 서비스) |

### 단축키

| 동작 | 키 |
|---|---|
| 선택한 서비스 시작 | `Shift+F10` (트리에서 더블클릭 또는 `Enter`도 가능) |
| 중지 | `Ctrl+F2` |
| 재시작 | `Ctrl+F5` |
| 서비스 검색 | `Ctrl+K` |
| 이름 바꾸기 / 삭제 | `F2` / `Delete` (트리에서) |
| 설정 저장 | `Ctrl+S` (설정 탭에서) |
| 콘솔 검색 / 다음·이전 | `Ctrl+F` / `Enter`·`Shift+Enter` (콘솔에서) |
| 콘솔 지우기 | `Ctrl+L` (콘솔에서) |

## 설치

GitHub Actions가 `main`에 올라올 때마다 Windows 설치본(NSIS)을 만든다. [Actions](https://github.com/hulbo/ServicesDashboard/actions) 탭의 최신 `build` 실행에서 `services-dashboard-windows` 아티팩트를 받아 설치한다.

직접 빌드하려면 아래 [개발](#개발)을 본다.

## 사용법

### 프로젝트에서 가져오기 (권장)

1. 왼쪽 위 **가져오기**(⤓)를 누르고 프로젝트 폴더(`.idea`가 있는 루트)를 고른 뒤 **스캔**한다.
2. 후보가 출처별(IntelliJ 실행 구성 / npm 스크립트 / Spring Boot 모듈)로 나온다. 각 후보에 실제 실행 명령이 미리 보이고, 문제가 있으면 빨간 글씨나 경고로 알려 준다.
3. 필요한 것만 체크하고 이름·그룹·스크립트·프로파일을 고친 뒤 **가져오기**를 누른다. *프로젝트 경로를 변수로 저장*을 켜 두면 작업 디렉터리가 `${var:MY_PROJECT}/...`로 저장돼 폴더를 옮겨도 변수 하나만 고치면 된다.

| IntelliJ 구성 | 변환 결과 |
|---|---|
| Spring Boot, Application | Gradle 프로젝트면 `gradle-boot`(모듈 → `:apps:portal`), Maven이면 `maven-boot`(artifactId → `-pl` 모듈). 메인 클래스·프로파일·VM 옵션·인자·환경변수·*구성 속성 재정의* 포함 |
| npm | `npm` (package.json 위치, 스크립트, 인자, 패키지 매니저, 프로젝트 Node 인터프리터) |
| Node.js, Jar, Maven, Gradle, Shell Script | `node`, `jar`, `maven`/`maven-boot`, `gradle`/`gradle-boot`, `shell` |
| 폴더(`folderName`) | 그룹 |
| 프로젝트 SDK·대체 JRE | IntelliJ JDK 목록이나 감지된 JDK에서 실제 경로를 찾아 지정 |
| 복합(Compound) | 아직 지원하지 않음 (M4 실행 프리셋) |

### 직접 추가

1. 왼쪽 위 **+** 로 서비스를 추가하면 설정 탭이 열린다.
2. 실행 타입과 작업 디렉터리를 고르고 타입별 항목을 채운다. 아래쪽 **실행 명령 미리보기**에서 실제로 실행될 명령을 확인한다.
3. 저장(Ctrl+S) 후 **시작**을 누르면 콘솔 탭에 로그가 나온다.
4. 여러 프로젝트가 같은 루트를 쓰면 왼쪽 위 **워크스페이스 설정**에서 `ROOT` 같은 변수를 만들고 작업 디렉터리에 `${var:ROOT}/front/portal`처럼 쓴다.

### 타입별 실행 명령

| 타입 | 실행되는 명령 (예) |
|---|---|
| `npm` | `npm.cmd run dev -- <인자>` |
| `gradle-boot` | `gradlew.bat --console=plain -I <init script> :apps:portal:bootRun "--args=--spring.profiles.active=local --server.port=8081"` |
| `maven-boot` | `mvn.cmd -pl portal spring-boot:run -Dspring-boot.run.profiles=local -Dspring-boot.run.jvmArguments=...` |
| `jar` | `<JDK>\bin\java.exe <VM 옵션> -jar app.jar <인자>` |
| `shell` (cmd) | `cmd.exe /d /s /c "<명령>"` |

Gradle Wrapper(`gradlew.bat`)와 Maven Wrapper(`mvnw.cmd`)는 작업 디렉터리와 상위 폴더에서 찾아 우선 사용한다.

Gradle은 기본으로 `--no-daemon`으로 실행한다. 그래야 빌드 JVM과 앱이 모두 서비스의 프로세스 묶음(Job) 안에 들어가 중지할 때 바로 함께 종료된다. 설정에서 *데몬 재사용*을 켜면 시작이 빨라지지만, IntelliJ 등이 띄운 데몬에 붙으면 중지 후 앱 종료가 몇 초 늦을 수 있다.

## 설정 파일

서비스 정의는 `%APPDATA%\ServicesDashboard\workspaces\default.json`에 JSON으로 저장된다. 화면에서 편집하는 것이 기본이고, [JSON Schema](schema/workspace.schema.json)가 있어 에디터 자동완성으로 직접 고칠 수도 있다 (고친 뒤 워크스페이스 설정의 **다시 불러오기**). 파일이 깨져 있으면 덮어쓰지 않고 오류를 보여 준다.

```json
{
  "name": "my-project",
  "variables": { "ROOT": "C:/Project/my-project" },
  "groups": [{ "id": "server", "name": "server", "env": { "SPRING_PROFILES_ACTIVE": "local" } }],
  "services": [
    {
      "id": "portal-api",
      "name": "PortalApplication (local)",
      "group": "server",
      "type": "gradle-boot",
      "cwd": "${var:ROOT}",
      "gradle": { "project": ":apps:portal" },
      "jvmArgs": ["--add-opens", "java.base/java.lang=ALL-UNNAMED"],
      "runtime": { "jdk": "C:/Program Files/Java/jdk-17" },
      "ports": [8080]
    }
  ]
}
```

전체 예시: [examples/workspace.example.json](examples/workspace.example.json). 환경변수 `SERVICES_DASHBOARD_HOME`을 지정하면 설정 폴더 위치를 바꿀 수 있다 (포터블 사용).

## 개발

필요한 도구:

- Node.js 20 이상
- Rust stable (`rustup`, MSVC 툴체인)
- Visual Studio C++ Build Tools, WebView2 (Windows 11 기본 포함)

```powershell
npm install
npm run tauri dev     # 앱 개발 실행
npm run tauri build   # 설치본 빌드 → src-tauri/target/release/bundle/nsis/
```

```powershell
npm run check                                   # Svelte·TypeScript 타입 검사
npm test                                        # 프론트엔드 단위 테스트 (vitest)
cargo test --manifest-path src-tauri/Cargo.toml # 백엔드 단위·통합 테스트
```

`npm run dev`로 띄운 화면을 일반 브라우저에서 열면 **가짜 백엔드**(샘플 서비스와 가짜 로그)로 동작한다. 화면만 고칠 때 편하다. 실제 프로세스는 실행하지 않는다.

### 구조

```
src/                       프론트엔드 (Svelte 5 + TypeScript)
  lib/api.ts               백엔드 호출 (Tauri / 브라우저용 가짜 백엔드)
  lib/state.svelte.ts      앱 상태
  lib/ansi.ts              ANSI 색상 파서
  lib/logstore.ts          콘솔 로그 버퍼
  lib/importing.ts         가져오기 후보 → 워크스페이스 반영 (변수 치환, 다시 가져오기 병합)
  lib/console-view.ts      콘솔 레벨·스택트레이스 접기·검색·필터·링크 계산
  lib/components/          ServiceTree, ConsoleView, ServiceForm, ImportPanel, RuntimesPanel, ...
src-tauri/src/             백엔드 (Rust, Tauri 2)
  model.rs                 워크스페이스 JSON 모델
  launch.rs                서비스 정의 → 실행 명령
  process.rs               실행·중지·상태 관리
  job.rs                   Windows Job Object (프로세스 트리 묶기)
  runtime.rs               JDK·Node·Maven 감지와 버전 별칭 해석
  watch.rs                 준비 감지(로그 패턴·포트·HTTP), 포트·URL 감지
  ports.rs                 포트 점유 프로세스 조회와 종료
  logfile.rs               로그 파일 저장·내보내기
  shell.rs                 트레이·창 닫기·알림
  importers/               IntelliJ 실행 구성·package.json·Spring Boot 모듈 스캔
  logs.rs                  링 버퍼, 줄 조립, 인코딩 해석
  store.rs                 설정 파일 저장
  commands.rs              IPC 명령
schema/                    워크스페이스 JSON Schema
docs/PLANNING.md           기획서
```

## 로드맵

| 단계 | 내용 | 상태 |
|---|---|---|
| M0 | 프로젝트 구성, CI, 워크스페이스 스키마 | 완료 |
| M1 | MVP: 트리 + 콘솔, npm / shell / Spring Boot(Gradle·Maven) / jar 실행, 트리 종료, 설정 저장 | 완료 |
| M2 | IntelliJ·package.json·Spring Boot 가져오기, JDK / Node / Maven 런타임 감지, 인코딩·색상 자동 주입 | 완료 |
| M3 | 로그 검색·필터, 준비·포트 감지, 포트 충돌 처리, 트레이·알림, 로그 파일 | 완료 |
| M4 | 의존 순서 실행, 프리셋, 자동 재시작, 디버그 모드, 빠른 java 기동 | 예정 |

자세한 범위와 완료 기준은 [기획서 9장](docs/PLANNING.md#9-마일스톤)에 있다.

### 알려진 제한

- 중지는 프로세스 트리를 바로 종료한다. 정상 종료 신호(Ctrl+Break)를 먼저 보내는 단계는 아직 없다.
- 포트 충돌 검사는 설정한 포트와 이전 실행에서 감지한 포트만 본다. 처음 실행하는 서비스가 로그에만 포트를 알리는 경우는 실행 전에 알 수 없다.
- Windows 알림은 설치본으로 실행할 때 앱 이름으로 표시된다 (개발 실행에서는 다른 이름으로 보일 수 있음).

## 라이선스

[MIT](LICENSE)
