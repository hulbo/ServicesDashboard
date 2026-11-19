# 개발 가이드

Services Dashboard를 직접 빌드하거나 고치려는 사람을 위한 문서입니다. 사용법은 [사용 설명서](MANUAL.md), 기획 배경은 [기획서](PLANNING.md)를 보세요.

## 기술 스택

| 영역 | 사용 기술 |
|---|---|
| 앱 셸 | [Tauri 2](https://tauri.app/) (Windows WebView2) |
| 백엔드 | Rust (프로세스 관리, Job Object, 로그 수집, 가져오기, git) |
| 화면 | Svelte 5 + TypeScript, Vite |
| 테스트 | `cargo test`, vitest, svelte-check |
| 빌드·배포 | GitHub Actions → NSIS 설치본 → GitHub Releases |

## 준비

- Node.js 20 이상
- Rust stable (`rustup`, MSVC 툴체인)
- Visual Studio C++ Build Tools, WebView2 (Windows 11 기본 포함)

## 빌드와 실행

```powershell
npm install
npm run tauri dev     # 앱 개발 실행
npm run tauri build   # 설치본 빌드 → src-tauri/target/release/bundle/nsis/
```

## 검사와 테스트

```powershell
npm run check                                   # Svelte·TypeScript 타입 검사
npm test                                        # 프론트엔드 단위 테스트 (vitest)
cargo test --manifest-path src-tauri/Cargo.toml # 백엔드 단위·통합 테스트
```

## 브라우저 미리보기

`npm run dev`로 띄운 화면(`http://localhost:1420`)을 일반 브라우저에서 열면 **가짜 백엔드**(`src/lib/mock.ts`: 샘플 서비스와 가짜 로그)로 동작합니다. 실제 프로세스는 실행하지 않으므로 화면만 고칠 때 편합니다. 문서의 스크린샷도 이 화면으로 찍었습니다.

## 설정 폴더 분리

환경변수 `SERVICES_DASHBOARD_HOME`에 폴더를 지정하면 설정을 그 폴더에 둡니다. 이때는 단일 인스턴스 검사를 건너뛰므로, 평소 쓰는 설치본을 켜 둔 채 개발 빌드를 따로 띄워 시험할 수 있습니다.

## 코드 구조

```
src/                       화면 (Svelte 5 + TypeScript)
  lib/api.ts               백엔드 호출 (Tauri / 브라우저용 가짜 백엔드)
  lib/mock.ts              브라우저 미리보기용 가짜 백엔드
  lib/state.svelte.ts      앱 상태
  lib/ansi.ts              ANSI 색상 파서
  lib/logstore.ts          콘솔 로그 버퍼
  lib/console-view.ts      콘솔 레벨·스택트레이스 접기·검색·필터·링크 계산
  lib/importing.ts         가져오기 후보 → 워크스페이스 반영 (변수 치환, 다시 가져오기 병합)
  lib/git.ts               브랜치 표시·밖에서 바꾼 브랜치 감지
  lib/reorder.ts           목록 순서 바꾸기·이름순 정렬
  lib/components/          ServiceTree, ConsoleView, ServiceForm, ImportPanel, BranchPanel, ...
src-tauri/src/             백엔드 (Rust, Tauri 2)
  model.rs                 워크스페이스 JSON 모델
  store.rs                 설정 파일 저장
  commands.rs              IPC 명령
  launch.rs                서비스 정의 → 실행 명령
  process.rs               실행·중지·상태 관리
  job.rs                   Windows Job Object (프로세스 트리 묶기)
  logs.rs                  링 버퍼, 줄 조립, 인코딩 해석
  logfile.rs               로그 파일 저장·내보내기
  watch.rs                 준비 감지(로그 패턴·포트·HTTP), 포트·URL 감지
  ports.rs                 포트 점유 프로세스 조회와 종료
  runtime.rs               JDK·Node·Maven 감지와 버전 별칭 해석
  javamain.rs              java-main 클래스패스 계산·캐시
  git.rs                   git 저장소 찾기·현재 브랜치·브랜치 목록·전환·fetch·pull
  shell.rs                 트레이·창 닫기·알림
  importers/               IntelliJ 실행 구성·package.json·Spring Boot 모듈·VS Code·공유 파일
schema/                    워크스페이스 JSON Schema
examples/                  워크스페이스 예시
docs/                      사용 설명서, 개발 가이드, 기획서, 스크린샷
```

## CI와 배포

- `main`에 push하면 GitHub Actions가 검사·테스트를 돌리고 설치본을 빌드합니다. 결과물은 실행 기록의 `services-dashboard-windows` 아티팩트로 받을 수 있습니다.
- 릴리스는 버전(`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`)을 올려 push한 뒤, 그 빌드의 설치본을 `v<버전>` 태그의 GitHub Release로 올립니다.
