// 사이드바 자동 숨김: 왼쪽 기둥(실행 중인 서비스 아이콘), 마우스를 올리면 펼치고 벗어나면 접기,
// 펼쳐지는 중 목록 자리로 점프·정지해도 유지, 창 밖으로 나가면 접힘, Ctrl+B·Ctrl+K·Esc,
// 기둥 아이콘 테두리 색 = 통합 로그의 서비스 이름 색
import { createSuite, sleep } from './lib.mjs'

const t = createSuite('autohide')
const keep = (msg) => ({ command: `node -e "console.log('${msg}'); setInterval(() => {}, 1000)"` })
t.writeSettings()
t.writeWorkspace({
  groups: [
    { id: 'g1', name: 'server' },
    { id: 'g2', name: 'batch' },
  ],
  services: [
    { id: 'portal', name: 'PortalApplication (local)', group: 'g1', type: 'shell', shell: keep('portal up') },
    { id: 'auth', name: 'AuthApplication', group: 'g1', type: 'shell', shell: keep('auth up') },
    { id: 'order', name: '주문 배치', group: 'g2', type: 'shell', shell: keep('order up') },
    { id: 'admin', name: 'admin', group: 'g2', type: 'shell', shell: keep('admin up') },
  ],
})
const { check } = t

let app = await t.launch()
const sidebarOpen = () => app.evaluate(`(() => { const s = document.querySelector('.sidebar'); return getComputedStyle(s).visibility === 'visible' && s.getBoundingClientRect().left >= 0 })()`)
const sidebarClosed = async () => !(await sidebarOpen())
const hasRail = () => app.evaluate(`!!document.querySelector('.rail')`)
const railIcons = () => app.evaluate(`[...document.querySelectorAll('.rail .svc')].map(b => b.querySelector('.abbr').textContent)`)
const mainLeft = async () => Math.round((await app.rect('.main')).l)
const headName = () => app.evaluate(`document.querySelector('.panel h2, .head h2')?.textContent?.trim() ?? ''`)
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b)
const BODY = { x: 900, y: 450 }

/** 실패했을 때 보려고 기둥·목록의 enter/leave와 열림·닫힘을 기록한다 */
const recordEvents = () =>
  app.evaluate(`(() => {
    window.__ev = []
    const t0 = performance.now(), T = () => Math.round(performance.now() - t0)
    const push = (x) => { window.__ev.push(x); if (window.__ev.length > 200) window.__ev.shift() }
    for (const ty of ['mouseenter', 'mouseleave']) document.addEventListener(ty, (e) => { const c = e.target.classList; if (c && (c.contains('sidebar') || c.contains('rail'))) push([T(), ty, c.contains('sidebar') ? 'sidebar' : 'rail', Math.round(e.clientX), Math.round(e.clientY)]) }, true)
    let was = null
    setInterval(() => { const s = document.querySelector('.sidebar'); const o = s && s.classList.contains('open'); if (o !== was) { push([T(), o ? 'OPEN' : 'CLOSE']); was = o } }, 10)
    return true
  })()`)
const events = (n = 12) => app.evaluate(`JSON.stringify((window.__ev || []).slice(-${n}))`)

check('앱 시작', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 6`), 30000))
await recordEvents()
check('기본: 사이드바 고정 (기둥 없음)', !(await hasRail()) && (await mainLeft()) === 304, `본문 왼쪽 ${await mainLeft()}px`)
check(
  '도구 모음에 자동 숨기기 버튼 (모두 보임)',
  (await app.evaluate(`[...document.querySelectorAll('.toolbar .tool')].some(b => b.title.startsWith('사이드바 자동 숨기기')) && !document.querySelector('.toolbar .tool.more')`)) === true,
)

// 서비스 두 개 실행
await app.invoke('service_start', { id: 'portal', debug: false })
await app.invoke('service_start', { id: 'order', debug: false })
await app.waitFor(async () => (await app.invoke('service_statuses')).filter((s) => s.state === 'running' || s.state === 'starting').length === 2, 15000)

// 1) 자동 숨기기
await app.click('.toolbar .tool[title^="사이드바 자동 숨기기"]', 12)
await app.moveTo(700, 400)
check('자동 숨김: 기둥이 나타남', await app.waitFor(hasRail, 2000))
await sleep(400)
check('자동 숨김: 목록은 접힘', await sidebarClosed())
check('본문이 기둥 오른쪽부터 넓어짐', (await mainLeft()) === 44, `본문 왼쪽 ${await mainLeft()}px`)
check('기둥: 실행 중인 서비스 아이콘 2개 (트리 순서)', same(await railIcons(), ['PA', '주문']), JSON.stringify(await railIcons()))
check('기둥: 실행 수', (await app.evaluate(`document.querySelector('.rail .count').textContent.trim()`)) === '2')
const tip = await app.evaluate(`document.querySelector('.rail .svc').title`)
check('아이콘 설명: 이름 · 상태', tip.startsWith('PortalApplication (local) · '), tip)
check('설정 기억 (localStorage)', (await app.evaluate(`localStorage.getItem('sd.sidebarPinned')`)) === 'false')

// 2) 기둥에 마우스 → 펼침, 목록 위에 있으면 유지, 벗어나면 접힘
const rail = await app.rect('.rail')
const railSpot = { x: rail.l + rail.w / 2, y: rail.b - 80 }
await app.moveTo(railSpot.x, railSpot.y)
check('기둥에 마우스를 올리면 펼쳐짐', await app.waitFor(sidebarOpen, 1500))
await app.waitFor(async () => Math.round((await app.rect('.sidebar')).l) === 44, 1000) // 펼치는 애니메이션(0.16초)이 끝날 때까지
const sb = await app.rect('.sidebar')
check('목록은 기둥 오른쪽에 겹쳐서', Math.round(sb.l) === 44 && Math.round(sb.w) === 300 && (await mainLeft()) === 44, `left ${sb.l}, width ${sb.w}`)
await app.shot('autohide-01-open.png')
await app.moveTo(sb.l + 150, sb.t + 300)
await sleep(600)
check('목록 위로 옮겨도 열려 있음', await sidebarOpen(), await events())
await app.moveTo(BODY.x, BODY.y)
check('본문으로 벗어나면 접힘', await app.waitFor(sidebarClosed, 2000))

// 2-1) 펼쳐지는 동안(0.16초) 목록 자리로 한 번에 옮기고 멈춰도 열려 있어야 함 (목록이 멈춘 커서 아래로 들어옴)
for (const [i, lag] of [0, 0, 30, 60, 100].entries()) {
  await app.moveTo(railSpot.x, railSpot.y)
  await app.waitFor(app.ui(`document.querySelector('.sidebar').classList.contains('open')`), 1500)
  if (lag) await sleep(lag)
  const midLeft = Math.round((await app.rect('.sidebar')).l)
  await app.jumpTo(230, 400)
  await sleep(700)
  check(`펼치는 중 ${lag}ms에 목록 위로 점프·정지해도 열려 있음${i === 1 ? ' (다시)' : ''}`, await sidebarOpen(), `점프 때 목록 왼쪽 ${midLeft}px ${await events(6)}`)
  if (i === 0) {
    // 브라우저가 진입을 놓친 채 열려 있어도, 본문으로 한 번에 옮기면 접혀야 함
    await app.jumpTo(BODY.x, BODY.y)
    check('진입을 놓친 목록도 본문으로 옮기면 접힘', await app.waitFor(sidebarClosed, 2000))
  } else if (i === 1 || i === 2) {
    // 창 밖으로 나가도 접혀야 함 (i=1: 진입을 놓친 채, i=2: 정상 진입)
    const vw = await app.evaluate('innerWidth')
    await app.jumpTo(vw + 40, 400)
    check(`목록 위에서 창 밖으로 나가면 접힘 (${i === 1 ? '진입 놓침' : '정상 진입'})`, await app.waitFor(sidebarClosed, 2000), await events(4))
  }
  await app.moveTo(BODY.x, BODY.y)
  await app.waitFor(sidebarClosed, 2000)
  await sleep(300)
}

// 3) 펼친 목록에서 서비스 고르기
await app.moveTo(railSpot.x, railSpot.y)
await app.waitFor(sidebarOpen, 1500)
await sleep(250)
const adminRow = await app.evaluate(`(() => {
  const r = [...document.querySelectorAll('.list .row')].find(r => r.querySelector('.name')?.textContent.trim() === 'admin')
  const b = r.getBoundingClientRect()
  return { x: b.left + 60, y: b.top + b.height / 2 }
})()`)
await app.clickAt(adminRow.x, adminRow.y)
check('목록에서 서비스를 고름', await app.waitFor(async () => (await headName()) === 'admin', 3000), await headName())
await app.moveTo(BODY.x, BODY.y)
check('고른 뒤 본문으로 가면 접힘', await app.waitFor(sidebarClosed, 2000))

// 4) 기둥 아이콘 클릭 → 그 서비스 콘솔
await app.click('.rail .svc')
check('기둥 아이콘으로 서비스 열기', await app.waitFor(async () => (await headName()) === 'PortalApplication (local)', 3000), await headName())
check('고른 아이콘 강조', (await app.evaluate(`document.querySelector('.rail .svc').classList.contains('selected')`)) === true)
await app.moveTo(BODY.x, BODY.y)
await app.waitFor(sidebarClosed, 2000)

// 5) 검색 입력 중에는 마우스가 벗어나도 접지 않음, Ctrl+K로 열기
await app.press('Ctrl+K')
check('Ctrl+K: 펼치고 검색칸에 포커스', await app.waitFor(async () => (await sidebarOpen()) && (await app.evaluate(`!!document.activeElement?.matches('.filter input')`)), 2000))
await app.type('adm')
await sleep(600)
check('입력 중이면 마우스가 밖이어도 열려 있음', (await sidebarOpen()) && (await app.evaluate(`document.querySelectorAll('.list .row').length`)) <= 3)
await app.press('Escape')
check('검색칸에서 Esc는 목록을 접지 않음', await sidebarOpen())
await app.evaluate(`(() => { const i = document.querySelector('.filter input'); i.value = ''; i.dispatchEvent(new Event('input', { bubbles: true })); return true })()`)
await app.clickAt(BODY.x, BODY.y) // 본문을 누르면 포커스가 빠져 접힘
check('본문을 누르면 접힘', await app.waitFor(sidebarClosed, 2000))

// 5-1) 목록 위에 마우스가 있어도 Esc로 접기
await app.moveTo(railSpot.x, railSpot.y)
await app.waitFor(sidebarOpen, 1500)
await app.moveTo(sb.l + 150, sb.t + 300)
await sleep(300)
await app.press('Escape')
check('Esc로 접기', await app.waitFor(sidebarClosed, 1500))
await app.moveTo(BODY.x, BODY.y)

// 6) 서비스를 멈추면 기둥에서 빠짐
await app.invoke('service_stop', { id: 'order' })
check('멈춘 서비스는 기둥에서 빠짐', await app.waitFor(async () => same(await railIcons(), ['PA']), 8000), JSON.stringify(await railIcons()))
await app.invoke('service_start', { id: 'auth', debug: false })
check('새로 시작한 서비스는 기둥에 추가', await app.waitFor(async () => same(await railIcons(), ['PA', 'AA']), 8000), JSON.stringify(await railIcons()))
await app.shot('autohide-02-rail.png')

// 6-1) 기둥 아이콘 테두리 = 통합 로그의 서비스 이름 색
await app.clickRow('server')
await app.waitFor(app.ui(`[...document.querySelectorAll('.tabs button')].some(b => b.textContent.includes('통합 로그'))`), 3000)
await app.evaluate(`[...document.querySelectorAll('.tabs button')].find(b => b.textContent.includes('통합 로그')).click(); true`)
check(
  '통합 로그에 두 서비스 줄',
  await app.waitFor(app.ui(`(() => { const n = new Set([...document.querySelectorAll('.merged .who')].map(e => e.textContent.trim())); return n.has('PortalApplication (local)') && n.has('AuthApplication') })()`), 8000),
)
const colors = await app.evaluate(`(() => {
  const merged = {}; for (const e of document.querySelectorAll('.merged .who')) merged[e.textContent.trim()] = getComputedStyle(e).color
  const rail = {}; for (const b of document.querySelectorAll('.rail .svc')) rail[b.title.split(' · ')[0]] = getComputedStyle(b).borderTopColor
  return { merged, rail }
})()`)
const names = ['PortalApplication (local)', 'AuthApplication']
check('기둥 테두리 색 = 통합 로그 이름 색', names.every((n) => colors.merged[n] && colors.merged[n] === colors.rail[n]), JSON.stringify(colors))
check('두 서비스 색은 서로 다름', colors.rail[names[0]] !== colors.rail[names[1]])
await app.shot('autohide-03-colors.png')

// 7) 다시 켜도 자동 숨김 유지
await app.quit()
app = await t.launch()
check('다시 켬', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 6`), 30000))
check('다시 켜도 자동 숨김', (await app.waitFor(hasRail, 3000)) && (await mainLeft()) === 44)

// 8) 기둥의 고정 버튼 · Ctrl+B
await app.moveTo(BODY.x, BODY.y)
await app.click('.rail .tool[aria-label="사이드바 고정"]', 12)
check('기둥의 고정 버튼 → 사이드바 고정', await app.waitFor(async () => !(await hasRail()) && (await mainLeft()) === 304, 2000), `본문 왼쪽 ${await mainLeft()}px`)
await app.press('Ctrl+B')
check('Ctrl+B → 자동 숨김', await app.waitFor(hasRail, 2000))
await app.press('Ctrl+B')
check('Ctrl+B → 다시 고정', await app.waitFor(async () => !(await hasRail()) && (await sidebarOpen()), 2000))

await t.finish()
