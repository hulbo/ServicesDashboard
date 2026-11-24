// 설정 → 글꼴: 설치된 글꼴 목록, 화면·콘솔 글꼴과 크기, 바로 미리보기, 저장·되돌리기, 다시 켜도 유지, 콘솔 줄 높이
import { createSuite, sleep } from './lib.mjs'

const t = createSuite('fonts')
// 글꼴 설정이 없는 예전 설정 파일에서 시작
t.writeSettings({ logFiles: { enabled: false, retentionDays: 7 }, clearConsoleOnStart: true })
t.writeWorkspace({ services: [{ id: 'web', name: 'web', type: 'node', cwd: t.writeSampleApp(), node: { entry: 'server.js' } }] })
const { check } = t

let app = await t.launch()
const openSettings = () => app.clickTool('설정')
const pick = (sel, v) =>
  app.evaluate(`(() => { const el = document.querySelector(${JSON.stringify(sel)}); el.value = ${JSON.stringify(String(v))}; el.dispatchEvent(new Event('change', { bubbles: true })); return el.value })()`)
const treeFont = () => app.evaluate(`getComputedStyle(document.querySelector('.list .row .name')).fontFamily.split(',')[0].replace(/"/g, '').trim()`)
const rowVar = () => app.evaluate(`getComputedStyle(document.documentElement).getPropertyValue('--console-row').trim()`)
const saveBtn = `[...document.querySelectorAll('.head .btn')].find(b => b.textContent.includes('저장'))`
const resetBtn = `[...document.querySelectorAll('.btn')].find(b => b.textContent.includes('기본 글꼴로'))`
const disk = () => t.readSettings().fonts
const CHOSEN = { ui: 'Malgun Gothic', mono: 'Consolas', consoleSize: 14 }
const DEFAULT = { ui: '', mono: '', consoleSize: 12.5 }
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b)

check('앱 시작', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 1`), 30000))
const s0 = await app.invoke('settings_get')
check('설정 기본값 (예전 설정 파일)', same(s0.fonts, DEFAULT), JSON.stringify(s0.fonts))
check('기본 화면 글꼴', (await treeFont()) === 'Pretendard' && (await rowVar()) === '18px', `${await treeFont()} / ${await rowVar()}`)

// 1) 설치된 글꼴 목록
const fonts = await app.invoke('system_fonts')
const consolas = fonts.find((f) => f.name === 'Consolas')
const malgun = fonts.find((f) => f.name === 'Malgun Gothic')
const monoCount = fonts.filter((f) => f.mono).length
check('설치된 글꼴 목록', fonts.length > 50 && consolas?.mono === true && !!malgun, `${fonts.length}개, 고정폭 ${monoCount}개`)
await openSettings()
check('설정 화면: 화면 글꼴 목록', await app.waitFor(app.ui(`document.querySelector('#s-font-ui')?.options.length === ${fonts.length + 1}`), 10000))
const groups = await app.evaluate(`[...document.querySelectorAll('#s-font-mono optgroup')].map(g => g.children.length)`)
check('콘솔 글꼴: 고정폭을 위에 따로', same(groups, [monoCount, fonts.length - monoCount]), JSON.stringify(groups))
if (malgun?.local) {
  const label = `${malgun.local} (Malgun Gothic)`
  check('현지 이름도 표시', (await app.evaluate(`[...document.querySelector('#s-font-ui').options].some(o => o.textContent === ${JSON.stringify(label)})`)) === true, label)
}

// 2) 고르면 바로 미리보기, 저장하지 않고 떠나면 되돌림
await pick('#s-font-ui', CHOSEN.ui)
await pick('#s-font-mono', CHOSEN.mono)
await pick('#s-font-size', CHOSEN.consoleSize)
await sleep(200)
check('미리보기: 앱 전체에 바로 적용', (await treeFont()) === 'Malgun Gothic' && (await rowVar()) === '20px', `${await treeFont()} / ${await rowVar()}`)
check(
  '미리보기 칸: 콘솔 글꼴·크기',
  (await app.evaluate(`(() => { const c = getComputedStyle(document.querySelector('.mono-sample')); return c.fontFamily.replace(/"/g, '').startsWith('Consolas,') && c.fontSize === '14px' })()`)) === true,
)
check('저장 버튼 켜짐', (await app.evaluate(`${saveBtn}.disabled`)) === false)
await app.shot('fonts-01-settings.png')
await app.clickRow('web')
await sleep(300)
check('저장 안 하고 떠나면 원래대로', (await treeFont()) === 'Pretendard' && (await rowVar()) === '18px', `${await treeFont()} / ${await rowVar()}`)
check('파일은 그대로', !same(disk() ?? null, CHOSEN))

// 3) 저장
await openSettings()
await app.waitFor(app.ui(`!!document.querySelector('#s-font-ui')`))
check('다시 열면 저장된 값', (await app.evaluate(`document.querySelector('#s-font-ui').value + '|' + document.querySelector('#s-font-size').value`)) === '|12.5')
await pick('#s-font-ui', CHOSEN.ui)
await pick('#s-font-mono', CHOSEN.mono)
await pick('#s-font-size', CHOSEN.consoleSize)
await app.evaluate(`${saveBtn}.click(); true`)
check('저장 → 설정 파일', await app.waitFor(async () => same(disk(), CHOSEN), 5000), JSON.stringify(disk()))
await app.clickRow('web')
await sleep(300)
check('저장 후 다른 화면에서도 유지', (await treeFont()) === 'Malgun Gothic' && (await rowVar()) === '20px')

// 4) 콘솔: 글꼴·줄 높이, 가상 스크롤 맨 끝
await app.waitFor(app.ui(`!!document.querySelector('.btn.run')`))
await app.evaluate(`document.querySelector('.btn.run').click(); true`)
check('web 실행', await app.waitFor(async () => (await app.logTexts('web')).includes('READY-MARK'), 15000))
await sleep(500)
const con = await app.evaluate(
  `(() => { const v = document.querySelector('.console .viewport'); const l = v.querySelector('.line'); return { font: getComputedStyle(v).fontFamily.split(',')[0].replace(/"/g, ''), size: getComputedStyle(v).fontSize, h: getComputedStyle(l).height } })()`,
)
check('콘솔 글꼴·크기·줄 높이', con.font === 'Consolas' && con.size === '14px' && con.h === '20px', JSON.stringify(con))
const total = (await app.logTexts('web')).length
await app.evaluate(`(() => { const v = document.querySelector('.console .viewport'); v.scrollTop = v.scrollHeight; return true })()`)
await sleep(400)
const end = await app.evaluate(`(() => {
  const v = document.querySelector('.console .viewport'); const box = v.querySelector('.spacer-box'); const lines = [...v.querySelectorAll('.line')]
  const vr = v.getBoundingClientRect()
  const visible = lines.filter(l => { const r = l.getBoundingClientRect(); return r.bottom <= vr.bottom + 1 && r.top >= vr.top - 1 })
  return { boxH: box.getBoundingClientRect().height, last: visible.at(-1)?.textContent.trim(), steps: lines.slice(0, 3).map(l => Math.round(l.getBoundingClientRect().top)) }
})()`)
// 스택트레이스 8줄 중 일부는 접혀 화면 행 수가 로그 줄 수보다 적다
const shown = Math.round(end.boxH) / 20
check('가상 스크롤 높이 = 화면 행 수 × 20px', Number.isInteger(shown) && shown <= total && shown >= total - 8, `${end.boxH}px = ${shown}행 / 로그 ${total}줄`)
check('맨 끝까지 내리면 마지막 줄이 보임', end.last === 'READY-MARK', end.last)
check('줄 간격 20px', end.steps[1] - end.steps[0] === 20 && end.steps[2] - end.steps[1] === 20, JSON.stringify(end.steps))
await app.shot('fonts-02-console.png')

// 5) 다시 켜도 유지
await app.quit()
app = await t.launch()
check('다시 켬', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 1`), 30000))
check('다시 켜도 저장한 글꼴', await app.waitFor(async () => (await treeFont()) === 'Malgun Gothic' && (await rowVar()) === '20px', 5000), `${await treeFont()} / ${await rowVar()}`)

// 6) 기본 글꼴로
await openSettings()
await app.waitFor(app.ui(`!!document.querySelector('#s-font-ui')`))
await app.evaluate(`${resetBtn}.click(); true`)
await sleep(200)
check('기본 글꼴로: 미리보기', (await treeFont()) === 'Pretendard' && (await rowVar()) === '18px')
await app.evaluate(`${saveBtn}.click(); true`)
check('기본 글꼴 저장', await app.waitFor(async () => same(disk(), DEFAULT), 5000), JSON.stringify(disk()))
check('기본 글꼴로 버튼 꺼짐', (await app.evaluate(`${resetBtn}.disabled`)) === true)

// 7) 잘못된 값은 저장할 때 바로잡음
const s1 = await app.invoke('settings_get')
const fixed = await app.invoke('settings_save', { settings: { ...s1, fonts: { ui: '  Consolas ', mono: '', consoleSize: 99 } } })
check('잘못된 크기·공백 바로잡음', fixed.fonts.ui === 'Consolas' && fixed.fonts.consoleSize === 24, JSON.stringify(fixed.fonts))
await app.invoke('settings_save', { settings: s1 })

await t.finish()
