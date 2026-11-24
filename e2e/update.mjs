// 앱 안 업데이트: 자동 확인과 상태 표시줄 버튼, 설정의 '지금 확인'·최신·실패 표시,
// 받기와 서명 확인, 변조된 파일 거부, 서명된 버전 ≠ 알린 버전 거부, 자동 확인 끄기
//
// - 로컬 http 서버가 latest.json과 '설치본'을 내놓고, 앱은 SERVICES_DASHBOARD_UPDATE_URL로 그 주소를 읽는다.
//   릴리스 빌드는 http 주소를 받지 않으므로 디버그 빌드를 쓴다.
// - '설치본'은 임의의 바이트에 업데이트 서명 키로 서명한 파일이다. 서명 확인은 실제와 똑같이 하지만 실행할 수 없는 파일이라
//   혹시 설치 단계가 불려도 아무것도 덮어쓰지 않는다. 그래도 설치(update_install)는 부르지 않는다.
// - 서명 키(기본 ~/.tauri/services-dashboard-updater.key, SD_UPDATER_KEY로 바꿈)가 없거나 앱의 공개 키와 짝이 아니면 건너뛴다.
import crypto from 'node:crypto'
import fs from 'node:fs'
import http from 'node:http'
import os from 'node:os'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { DEBUG_EXE, REPO, createSuite, sleep } from './lib.mjs'

const t = createSuite('update')
const exe = process.env.SD_DEBUG_EXE || DEBUG_EXE
const keyPath = process.env.SD_UPDATER_KEY || path.join(os.homedir(), '.tauri', 'services-dashboard-updater.key')
if (!fs.existsSync(exe)) t.skip(`디버그 빌드가 없습니다: ${exe} (npm run tauri build -- --debug --no-bundle)`)
if (!fs.existsSync(keyPath)) t.skip(`업데이트 서명 키가 없습니다: ${keyPath}`)
const appPubkey = JSON.parse(fs.readFileSync(path.join(REPO, 'src-tauri', 'tauri.conf.json'), 'utf8')).plugins.updater.pubkey
if (fs.existsSync(`${keyPath}.pub`) && fs.readFileSync(`${keyPath}.pub`, 'utf8').trim() !== appPubkey.trim()) {
  t.skip(`이 서명 키는 앱의 공개 키와 짝이 아닙니다: ${keyPath}`)
}

const NEW = '99.0.0'
const MISMATCH = '98.0.0'

// ── 서명한 '설치본' ──
const setup = t.path('setup.exe')
fs.writeFileSync(setup, crypto.randomBytes(512 * 1024))
const signEnv = { ...process.env, TAURI_SIGNING_PRIVATE_KEY_PASSWORD: '' }
delete signEnv.TAURI_SIGNING_PRIVATE_KEY
delete signEnv.TAURI_SIGNING_PRIVATE_KEY_PATH
execFileSync(process.execPath, [path.join(REPO, 'node_modules', '@tauri-apps', 'cli', 'tauri.js'), 'signer', 'sign', '-f', keyPath, '--app-version', NEW, setup], {
  env: signEnv,
  stdio: 'pipe',
})
if (!fs.existsSync(`${setup}.sig`)) throw new Error('서명 파일이 만들어지지 않았습니다')
const setupBytes = fs.readFileSync(setup)
const tampered = Buffer.from(setupBytes)
tampered[tampered.length - 100] ^= 0xff

// ── 로컬 업데이트 서버 ──
const notes = t.path('notes.txt')
fs.writeFileSync(notes, '시험용 새 버전입니다.\n- 서명 확인 시험')
let port = 0
const makeLatest = (version, file) => {
  const out = t.path(`latest-${version}-${file}.json`)
  execFileSync(process.execPath, [path.join(REPO, 'scripts', 'latest-json.mjs'), version, `${setup}.sig`, notes, out, `http://127.0.0.1:${port}/${file}`])
  return fs.readFileSync(out)
}
const latest = {}
let mode = 'new' // new | tampered | mismatch | same | 404
let hits = 0
const server = http.createServer((req, res) => {
  hits++
  if (req.url === '/latest.json') {
    if (mode === '404') return res.writeHead(404).end('not found')
    return res.writeHead(200, { 'content-type': 'application/json' }).end(latest[mode])
  }
  if (req.url === '/setup.exe') return res.writeHead(200, { 'content-length': setupBytes.length }).end(setupBytes)
  if (req.url === '/tampered.exe') return res.writeHead(200, { 'content-length': tampered.length }).end(tampered)
  res.writeHead(404).end()
})
await new Promise((r) => server.listen(0, '127.0.0.1', r))
port = server.address().port
latest.new = makeLatest(NEW, 'setup.exe')
latest.tampered = makeLatest(NEW, 'tampered.exe')
// NEW로 서명된 설치본을 MISMATCH라고 내놓음 (latest.json만 바꿔치기한 경우)
latest.mismatch = makeLatest(MISMATCH, 'setup.exe')

t.writeSettings({ autoUpdateCheck: true })
t.writeWorkspace({ services: [{ id: 'web', name: 'web', type: 'shell', shell: { command: `node -e "setInterval(() => {}, 1000)"` } }] })
const env = { SERVICES_DASHBOARD_UPDATE_URL: `http://127.0.0.1:${port}/latest.json` }
const { check } = t

let app = await t.launch({ exe, env })
const barText = () => app.evaluate(`document.querySelector('.statusbar .update')?.textContent.trim() ?? ''`)
const statusText = () => app.evaluate(`document.querySelector('.upd-status')?.textContent.trim() ?? ''`)
const clickCheck = () => app.clickButton('지금 확인', '.btn')
const newBox = () => app.evaluate(`document.querySelector('.upd-new')?.innerText ?? ''`)

// 1) 앱을 켜면 자동으로 확인 → 상태 표시줄에 업데이트 버튼
check('앱 시작', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 1`), 30000))
const current = (await app.invoke('app_info')).version
latest.same = makeLatest(current, 'setup.exe')
const t0 = Date.now()
check(`자동 확인 → 상태 표시줄에 "v${NEW} 업데이트"`, await app.waitFor(async () => (await barText()) === `v${NEW} 업데이트`, 15000), `${((Date.now() - t0) / 1000).toFixed(1)}초, "${await barText()}"`)
check('업데이트 서버에 요청이 갔음', hits >= 1, `${hits}번`)

// 2) 설정 화면: 새 버전 안내·노트
await app.clickTool('설정')
check(
  '설정: 새 버전 안내와 노트',
  await app.waitFor(async () => (await newBox()).includes(`v${NEW}`) && (await app.evaluate(`!!document.querySelector('.upd-new .notes')?.textContent.includes('서명 확인 시험')`)), 3000),
)
await app.shot('update-01-available.png')

// 3) 받기 + 서명 확인 (설치는 하지 않음)
const dl = await app.tryInvoke('update_download')
check('받기와 서명 확인 성공', dl.ok, dl.ok ? '' : dl.e)

// 4) 변조된 설치본은 거부
mode = 'tampered'
await clickCheck()
await app.waitFor(async () => (await newBox()) !== '' && !(await statusText()).includes('확인 중'), 5000)
const bad = await app.tryInvoke('update_download')
check('변조된 설치본: 서명이 맞지 않아 거부', !bad.ok && /서명|signature/i.test(bad.e), bad.ok ? '받아졌음!' : bad.e.slice(0, 160))

// 5) 서명된 버전과 latest.json의 버전이 다르면 거부
mode = 'mismatch'
await clickCheck()
await app.waitFor(async () => (await newBox()).includes(`v${MISMATCH}`), 5000)
const mis = await app.tryInvoke('update_download')
check(`서명된 버전(${NEW}) ≠ 알린 버전(${MISMATCH}): 거부`, !mis.ok && mis.e.includes(NEW) && mis.e.includes(MISMATCH), mis.ok ? '받아졌음!' : mis.e.slice(0, 160))

// 6) 이미 최신
mode = 'same'
await clickCheck()
check('최신이면 "최신 버전입니다."', await app.waitFor(async () => (await statusText()) === '최신 버전입니다.', 8000), await statusText())
check('최신이면 상태 표시줄 버튼 없음', (await barText()) === '')

// 7) 확인 실패 (서버 404): 직접 확인은 화면에 표시, 상태 표시줄은 조용
mode = '404'
await clickCheck()
check('직접 확인 실패는 화면에 표시', await app.waitFor(app.ui(`!!document.querySelector('.upd-status.err')`), 8000), await statusText())
check('실패해도 상태 표시줄 버튼 없음', (await barText()) === '')
await app.shot('update-02-error.png')
await app.quit()

// 8) 자동 확인 끄기: 켜도 확인하지 않음
mode = 'new'
t.writeSettings({ ...t.readSettings(), autoUpdateCheck: false })
hits = 0
app = await t.launch({ exe, env })
check('다시 켬 (자동 확인 끔)', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 1`), 30000))
await sleep(8000)
check('자동 확인을 끄면 요청하지 않음', hits === 0 && (await barText()) === '', `요청 ${hits}번`)

server.close()
await t.finish()
