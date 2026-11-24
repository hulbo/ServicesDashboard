// 콘솔: Ctrl+A는 콘솔 내용만 전체 선택, Ctrl+C 복사(접힌 스택트레이스 포함), Esc·클릭으로 해제,
// 트리·검색칸의 Ctrl+A, 직접 시작할 때 이전 로그 지우기와 그 설정
import { createSuite, sleep } from './lib.mjs'

const t = createSuite('console')
t.writeSettings()
t.writeWorkspace({
  services: [
    { id: 'web', name: 'web', type: 'node', cwd: t.writeSampleApp(), node: { entry: 'server.js' } },
    { id: 'crash', name: 'crash', type: 'shell', shell: { command: 'echo crash-run & exit 5' }, autoRestart: { maxRetries: 2, delaySecs: 1 } },
  ],
})

const app = await t.launch()
const { check } = t
const texts = (id) => app.logTexts(id)
const runs = (lines) => lines.filter((x) => x.startsWith('▶')).length
const docSelection = () => app.evaluate(`window.getSelection().toString().length`)
const toastText = () => app.evaluate(`[...document.querySelectorAll('.toast')].map(t => t.textContent).join('|')`)
const selHint = () => app.evaluate(`document.querySelector('.sel-hint')?.textContent || ''`)
const allSelected = () => app.evaluate(`!!document.querySelector('.console .viewport.all-selected')`)
const headBtn = (label) => app.clickButton(label, '.head .btn')
const runBtn = () => app.evaluate(`document.querySelector('.btn.run').click(); true`)

check('앱 시작', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 2`), 30000))
// 실제 클립보드를 쓰지 않도록 복사한 글자를 가로챈다
await app.evaluate(`window.__copied = null; navigator.clipboard.writeText = async (t) => { window.__copied = t }; true`)

// 1) 콘솔 Ctrl+A → 콘솔 내용만 선택
await app.clickRow('web')
await app.waitFor(app.ui(`!!document.querySelector('.btn.run')`))
await runBtn()
check('web 실행 · 로그 수신', await app.waitFor(async () => (await texts('web')).includes('READY-MARK'), 15000))
await sleep(400)
await app.click('.console .viewport')
await app.press('Ctrl+A')
check('콘솔 전체 선택 표시', await app.waitFor(app.ui(`!!document.querySelector('.console .viewport.all-selected') && !!document.querySelector('.sel-hint')`), 3000))
check('화면 전체(트리·버튼)는 선택되지 않음', (await docSelection()) === 0, `선택된 글자 ${await docSelection()}`)
const total = (await texts('web')).length
const hint = await selHint()
check('선택 줄 수 = 콘솔 줄 수 (접힌 스택트레이스 포함)', hint.includes(`전체 선택 ${total.toLocaleString()}줄`), `${hint.trim()} / 백엔드 ${total}`)
await app.shot('console-01-select-all.png')
await app.press('Ctrl+C')
check('Ctrl+C → 콘솔 내용 복사', await app.waitFor(app.ui(`typeof window.__copied === 'string'`), 3000))
const copied = (await app.evaluate(`window.__copied`)) || ''
const lines = copied.split('\n')
check('복사한 줄 수·처음·끝', lines.length === total && lines.includes('line 1') && lines.includes('READY-MARK'), `${lines.length}줄`)
check('접힌 프레임도 복사됨', lines.filter((l) => l.includes('at a.B.m')).length === 8)
check('트리·버튼 글자는 없음', !copied.includes('서비스 검색') && !copied.includes('crash') && !copied.includes('디버그'))
check('복사 알림', await app.waitFor(async () => (await toastText()).includes(`${total.toLocaleString()}줄을 클립보드에 복사했습니다`), 3000))
await app.press('Escape')
check('Esc로 선택 해제', await app.waitFor(async () => !(await allSelected()), 2000))
await app.press('Ctrl+A')
check('다시 Ctrl+A', await app.waitFor(allSelected, 2000))
await app.click('.console .viewport')
check('클릭하면 선택 해제', await app.waitFor(async () => !(await allSelected()), 2000))

// 2) 콘솔 밖(트리)에서 Ctrl+A → 아무것도 선택하지 않음
await app.click('.list')
await app.press('Ctrl+A')
check('트리에서 Ctrl+A: 화면 선택 없음', (await docSelection()) === 0, `선택된 글자 ${await docSelection()}`)
// 검색 입력칸에서는 원래대로 입력한 글자 전체 선택
const search = `document.querySelector('.console .search input')`
await app.click('.console .search input')
await app.evaluate(`(() => { const i = ${search}; i.value = 'abc'; i.dispatchEvent(new Event('input', { bubbles: true })); i.setSelectionRange(3, 3); return true })()`)
await app.press('Ctrl+A')
check(
  '검색칸 Ctrl+A는 입력 글자 선택',
  (await app.evaluate(`(() => { const i = ${search}; return i.selectionStart === 0 && i.selectionEnd === 3 })()`)) && !(await allSelected()),
)
await app.evaluate(`(() => { const i = ${search}; i.value = ''; i.dispatchEvent(new Event('input', { bubbles: true })); return true })()`)

// 3) 재시작하면 이전 로그를 지운다
check('재시작 전: 실행 1번', runs(await texts('web')) === 1)
const pid0 = (await app.statusOf('web')).pid
await headBtn('재시작')
check(
  '재시작 후 새 로그',
  await app.waitFor(async () => {
    const lines = await texts('web')
    return (await app.statusOf('web')).pid !== pid0 && lines.includes('READY-MARK') && runs(lines) === 1
  }, 15000),
)
let now = await texts('web')
check('백엔드: 이전 실행 줄 없음 (line 1이 한 번, 중지 줄 없음)', now.filter((x) => x === 'line 1').length === 1 && !now.some((x) => x.startsWith('■')), `${now.length}줄`)
await sleep(400)
await app.click('.console .viewport')
await app.press('Ctrl+A')
const hint2 = await selHint()
check('화면: 콘솔 줄 수 = 새 실행 줄 수', hint2.includes(`전체 선택 ${now.length.toLocaleString()}줄`), `${hint2.trim()} / ${now.length}`)
await app.press('Escape')

// 4) 중지 후 시작도 지운다
await headBtn('중지')
await app.waitFor(async () => (await app.statusOf('web')).state === 'stopped', 10000)
check('중지하면 로그는 남음', (await texts('web')).some((x) => x.startsWith('■')))
await runBtn()
check(
  '다시 시작 → 이전 로그 지움',
  await app.waitFor(async () => {
    const lines = await texts('web')
    return lines.includes('READY-MARK') && runs(lines) === 1 && !lines.some((x) => x.startsWith('■'))
  }, 15000),
)

// 5) 자동 재시작은 지우지 않는다 (실패 원인을 남긴다)
await app.clickRow('crash')
await app.waitFor(app.ui(`!!document.querySelector('.btn.run')`))
await runBtn()
check('자동 재시작 한도 도달', await app.waitFor(async () => (await texts('crash')).some((l) => l.includes('한도(2회)')), 20000))
const ct = await texts('crash')
const crashRuns = ct.filter((x) => x.trim() === 'crash-run').length
check('자동 재시작 3번의 로그가 모두 남음', crashRuns === 3 && runs(ct) === 3, `crash-run ${crashRuns}번 ${crashRuns === 3 ? '' : JSON.stringify(ct.slice(0, 6))}`)

// 6) 설정을 끄면 지우지 않는다
const s = await app.invoke('settings_get')
check('설정 기본값: 켜짐', s.clearConsoleOnStart === true)
await app.invoke('settings_save', { settings: { ...s, clearConsoleOnStart: false } })
await app.clickRow('web')
await app.waitFor(app.ui(`!!document.querySelector('.head .btn')`))
await headBtn('재시작')
check(
  '설정 끔: 이전 로그 유지 (실행 2번)',
  await app.waitFor(async () => {
    const lines = await texts('web')
    return runs(lines) === 2 && lines.filter((x) => x === 'READY-MARK').length === 2
  }, 15000),
)
await app.invoke('settings_save', { settings: s })

await t.finish()
