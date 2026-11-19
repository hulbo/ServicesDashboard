# Services Dashboard

IntelliJ를 켜지 않고 로컬 개발 서비스를 띄우는 **가벼운 데스크톱 서비스 대시보드**.

Node(npm), Spring Boot(Gradle / Maven), jar, 임의 명령으로 된 서비스를 한 화면에서 실행·중지·재시작하고, 서비스마다 콘솔 로그를 본다. IntelliJ의 Services 탭처럼 **왼쪽에는 서비스 트리, 오른쪽에는 콘솔 로그**를 둔다.

> **현재 상태: 기획 단계.** 아래 기능과 사용법은 구현 목표다. 상세 기획은 [docs/PLANNING.md](docs/PLANNING.md)에 있다.

## 왜 만드나

- 서비스 몇 개 띄우려고 무거운 IDE를 켜고 싶지 않다.
- 실행 설정이 `.run/*.run.xml`, `.idea/workspace.xml`, `package.json`, 프로파일, 환경변수, JDK 경로로 흩어져 있다.
- 터미널을 여러 개 띄우면 관리가 어렵고, 창을 닫아도 자식 프로세스가 남아 포트를 붙잡는다.

## 주요 기능 (계획)

| 기능 | 설명 |
|---|---|
| 서비스 트리 + 콘솔 | 그룹(폴더)별 서비스 목록과 상태 아이콘, 선택한 서비스의 실시간 로그 |
| 다양한 실행 타입 | `npm`, `node`, `gradle-boot`, `maven-boot`, `java-main`, `jar`, `gradle`, `maven`, `shell` |
| 설정 가져오기 | IntelliJ 실행 구성(`.run/*.run.xml`, `.idea/workspace.xml`), `package.json` scripts, Spring Boot 모듈 자동 감지 |
| 한 화면 설정 관리 | 환경변수, 프로파일, VM 옵션, 인자, 포트, 서비스별 JDK / Node 버전 |
| 확실한 종료 | Windows Job Object로 프로세스 트리 전체 종료. 대시보드가 꺼져도 고아 프로세스가 남지 않음 |
| 콘솔 로그 | ANSI 색상, 검색, 필터, 레벨 하이라이트, 스택트레이스 접기, 저장, 한글 인코딩 처리 |
| 운영 편의 | 준비 완료 감지, 포트 자동 감지와 브라우저 열기, 포트 충돌 처리, 의존 순서 실행, 트레이 상주, 알림 |
| 설정 우선 | 프로젝트 소스(`build.gradle`, `application.yml` 등)를 고치지 않고 인자·환경변수·init script로 해결 |

## 화면 구성

```
+------------------------------+-------------------------------------------------+
| [+] [Import] [>>] [■■]       | portal   ● 실행중   :5173   [브라우저 열기]     |
| v npm               2/5      |   VITE v5.4.0  ready in 812 ms                  |
|     ● portal       :5173     |   ➜  Local:   http://localhost:5173/            |
|     ○ admin                  |                                                 |
| v server            1/3      |                                                 |
|     ◐ PortalApi    starting  |                                                 |
|     ● AuthApi      :8081     |                                                 |
+------------------------------+-------------------------------------------------+
```

## 기술 스택

- [Tauri 2](https://tauri.app/) (Rust) — 프로세스 관리, Job Object, 로그 버퍼, 설정 저장, 가져오기
- [Svelte 5](https://svelte.dev/) + TypeScript + Vite — 트리, 콘솔 뷰어, 설정 폼
- 목표: 설치 파일 15MB 이하, 대기 메모리 100MB 이하

## 설정 파일 (계획)

서비스 정의는 `%APPDATA%\ServicesDashboard\workspaces\<이름>.json`에 저장한다. 화면에서 편집하는 것이 기본이고, JSON Schema를 제공해 에디터로 직접 고칠 수도 있다.

```json
{
  "name": "my-project",
  "variables": { "ROOT": "C:/Project/my-project" },
  "groups": [{ "id": "server", "name": "server" }],
  "services": [
    {
      "id": "portal-api",
      "name": "PortalApplication (local)",
      "group": "server",
      "type": "gradle-boot",
      "cwd": "${var:ROOT}",
      "gradle": { "project": ":apps:portal" },
      "spring": { "profiles": ["local"] },
      "runtime": { "jdk": "17" },
      "ports": [8080]
    }
  ]
}
```

전체 예시와 항목 설명은 [기획서 11.1](docs/PLANNING.md#111-워크스페이스-파일-예시)을 본다.

## 개발 (예정)

필요한 도구:

- Node.js 20 이상
- Rust stable (`rustup`)
- Windows: WebView2 (Windows 11 기본 포함), Visual Studio C++ Build Tools

```powershell
npm install
npm run tauri dev     # 개발 실행
npm run tauri build   # 설치본 빌드
```

## 로드맵

| 단계 | 내용 |
|---|---|
| M0 | 프로젝트 구성, CI, 워크스페이스 스키마 |
| M1 | MVP: 트리 + 콘솔, npm / shell / Spring Boot(Gradle·Maven) / jar 실행, 트리 종료, 설정 저장 |
| M2 | IntelliJ·package.json·Spring Boot 가져오기, JDK / Node 런타임 관리 |
| M3 | 로그 검색·필터, 준비·포트 감지, 포트 충돌 처리, 트레이·알림 |
| M4 | 의존 순서 실행, 프리셋, 자동 재시작, 디버그 모드, 빠른 java 기동 |

자세한 범위와 완료 기준은 [기획서 9장](docs/PLANNING.md#9-마일스톤)에 있다.

## 라이선스

[MIT](LICENSE)
