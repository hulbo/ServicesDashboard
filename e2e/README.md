# 실제 앱 E2E 시험

빌드한 앱을 따로 띄워 화면을 직접 조작하는 시험입니다. 앱은 WebView2 원격 디버깅(CDP)으로 조작하고, 마우스와 키보드도 실제 입력으로 보냅니다. 단위 테스트(`npm test`, `cargo test`)가 닿지 않는 곳을 확인합니다. 예를 들어 hover로 펼치고 접기, 끌어서 놓기, 키보드 단축키, 글꼴 적용, 앱 안 업데이트의 서명 확인입니다.

Windows에서 로컬로만 돌립니다. CI에서는 돌리지 않습니다.

## 준비

```powershell
npm run tauri build -- --no-bundle            # 릴리스 빌드: update를 뺀 모든 시험
npm run tauri build -- --debug --no-bundle    # 디버그 빌드: update 시험 (릴리스 빌드는 http 업데이트 주소를 받지 않음)
```

시험은 빌드된 실행 파일을 그대로 씁니다. 코드를 고쳤다면 다시 빌드한 뒤 돌리세요.

## 돌리기

```powershell
npm run e2e                     # 모두 차례로
npm run e2e -- console fonts    # 고른 것만
node e2e/autohide.mjs           # 하나만
```

| 시험 | 확인하는 것 |
|---|---|
| `console` | 콘솔 Ctrl+A는 콘솔 내용만 전체 선택, Ctrl+C 복사(접힌 스택트레이스 포함), Esc·클릭으로 해제, 트리·검색칸의 Ctrl+A, 직접 시작할 때 이전 로그 지우기(자동 재시작은 남김)와 그 설정 |
| `reorder` | 그룹·서비스 환경변수 편집기의 끌어서 순서 바꾸기, Alt+↑/↓, 이름순 정렬, 저장한 파일의 키 순서 유지 |
| `fonts` | 설치된 글꼴 목록, 화면·콘솔 글꼴과 크기, 바로 미리보기, 저장·되돌리기, 다시 켜도 유지, 콘솔 줄 높이와 가상 스크롤 |
| `toolbar` | 사이드바 폭이 좁으면 도구 모음 버튼이 ⋯ 메뉴로 넘어감, 다시 켜도 유지 |
| `autohide` | 사이드바 자동 숨김: 기둥의 실행 중 서비스 아이콘, 마우스로 펼치고 접기, 펼쳐지는 중 목록 자리로 점프·정지해도 유지, 창 밖으로 나가면 접힘, Ctrl+B·Ctrl+K·Esc, 기둥 테두리 색 = 통합 로그 이름 색 |
| `update` | 앱 안 업데이트: 자동 확인과 상태 표시줄 버튼, '지금 확인'·최신·실패 표시, 받기와 서명 확인, 변조된 파일 거부, 서명된 버전 ≠ 알린 버전 거부, 자동 확인 끄기 |

각 시험은 확인 항목마다 `PASS`/`FAIL`을 찍고 마지막에 `결과: 통과/전체`를 냅니다. 하나라도 실패하면 종료 코드가 1입니다.

## 동작 방식과 주의

- **설치본과 섞이지 않습니다.** 설정 폴더(`SERVICES_DASHBOARD_HOME`)와 WebView2 데이터 폴더는 OS 임시 폴더 아래 `sd-e2e-<시험>-*`에 매번 새로 만듭니다. 시작할 때 설정과 작업공간 파일을 직접 쓰므로 지난 실행 상태에 기대지 않습니다. 설정 폴더를 따로 쓰면 단일 인스턴스 검사도 건너뛰어서, 평소 쓰는 앱을 켜 둔 채 돌릴 수 있습니다.
- **자기가 띄운 앱만 끕니다.** 시험이 끝나거나 실패하면 시험이 띄운 PID만 종료합니다. 그 앱이 실행한 서비스는 앱의 Job Object가 함께 정리합니다.
- **시험 창은 최소화됩니다.** WebView2는 운영체제 마우스 메시지를 받으면 마우스 위치를 실제 커서로 다시 맞춥니다. 그래서 사용자 커서가 시험 창 위에서 움직이면 hover 시험이 깨집니다. 최소화해도 페이지는 계속 그려지고 CDP 입력도 동작합니다. 창을 보면서 돌리려면 `SD_SHOW=1`을 주되, 그동안 마우스를 창 위에 두지 마세요.
- **작업 폴더:** 모두 통과하면 지우고, 실패하면 남겨서 경로를 알려 줍니다. 스크린샷은 그 안 `shots/`에 있습니다. 통과해도 남기려면 `SD_KEEP=1`을 주세요.
- **업데이트 시험:** 로컬 http 서버가 `latest.json`과 '설치본'을 내놓습니다. 설치본은 임의의 바이트에 업데이트 서명 키로 서명한 파일이라 실행할 수 없습니다. 서명 확인은 실제와 똑같이 합니다. 설치 단계(`update_install`)는 실제 설치 폴더를 덮어쓰므로 부르지 않습니다. 서명 키가 없거나 키가 앱의 공개 키와 짝이 아니면 이 시험은 건너뜁니다. 서명 키 위치는 `~/.tauri/services-dashboard-updater.key`가 기본이고, `SD_UPDATER_KEY`로 바꿀 수 있습니다.
- **시험 데이터:** 저장소가 공개이므로, 시험에 넣는 서비스·경로·주소는 모두 가짜 값으로 둡니다.

## 환경변수

| 이름 | 뜻 | 기본값 |
|---|---|---|
| `SD_EXE` | 시험할 앱 (update 제외) | `src-tauri/target/release/services-dashboard.exe` |
| `SD_DEBUG_EXE` | update 시험에 쓸 디버그 빌드 | `src-tauri/target/debug/services-dashboard.exe` |
| `SD_UPDATER_KEY` | 업데이트 서명 키 | `~/.tauri/services-dashboard-updater.key` |
| `SD_SHOW` | 1이면 시험 창을 최소화하지 않음 | |
| `SD_KEEP` | 1이면 통과해도 작업 폴더를 남김 | |

## 새 시험 만들기

`e2e/<이름>.mjs`를 만들고 `run-all.mjs`의 목록에 넣습니다. 공통 도구는 `lib.mjs`에 있습니다.

```js
import { createSuite } from './lib.mjs'

const t = createSuite('example')
t.writeSettings()                                   // 자동 업데이트 확인은 기본으로 끔
t.writeWorkspace({ services: [{ id: 'a', name: 'a', type: 'shell', shell: { command: 'echo hi' } }] })
const app = await t.launch()                         // 앱을 띄우고 CDP 연결, 창 최소화
t.check('앱 시작', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 1`), 30000))
await app.clickRow('a')                              // 트리의 행 누르기
await app.press('Ctrl+K')                            // 키 입력
await app.click('.toolbar .tool.more')               // 실제 마우스로 누르기
t.check('상태', (await app.invoke('service_statuses')).length === 1)  // 백엔드 명령
await t.finish()                                     // 앱을 끄고 결과를 내고 종료
```
