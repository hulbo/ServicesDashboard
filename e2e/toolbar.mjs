// 사이드바 폭이 좁으면 도구 모음의 넘치는 버튼이 "더 보기(⋯)" 메뉴로 넘어간다
import { createSuite, sleep } from './lib.mjs'

const t = createSuite('toolbar')
t.writeSettings()
t.writeWorkspace({
  groups: [{ id: 'g', name: '개발_server' }],
  services: [{ id: 'a', name: 'Application', group: 'g', type: 'shell', shell: { command: 'echo a' } }],
})
const { check } = t

let app = await t.launch()
/** 사이드바 경계(스플리터)를 실제 마우스로 끌어 x 위치로 */
async function dragSidebar(x) {
  const r = await app.rect('.splitter')
  const cx = r.l + r.w / 2
  const cy = r.t + Math.min(r.h / 2, 200)
  await app.jumpTo(cx, cy)
  await app.mouse('mousePressed', cx, cy, 1)
  await app.moveTo(x, cy, 5, 1)
  await app.mouse('mouseReleased', x, cy)
  await sleep(400)
}
/** 보이는 도구 이름 (⋯ 포함)과, 잘려서 일부만 보이는 도구 수 */
const toolbar = () =>
  app.evaluate(`(() => {
    const tb = document.querySelector('.toolbar'); const r = tb.getBoundingClientRect()
    const tools = [...tb.querySelectorAll('.tool')]
    return {
      width: Math.round(document.querySelector('.sidebar').getBoundingClientRect().width),
      names: tools.map(b => b.classList.contains('more') ? '⋯' : b.title.split(' (')[0]),
      clipped: tools.filter(b => b.getBoundingClientRect().right > r.right - 1).length,
    }
  })()`)
const menuOpen = () => app.waitFor(app.ui(`!!document.querySelector('.menu')`), 3000)
const menuItem = (pred) => app.evaluate(`[...document.querySelectorAll('.menu .item')].find(b => ${pred}).click(); true`)

check('앱 시작', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 2`), 30000))
let tb = await toolbar()
const all = tb.names.length
check('기본 폭(300px): 도구 모두 보이고 더 보기 없음', all === 9 && !tb.names.includes('⋯') && tb.clipped === 0, `${tb.width}px ${tb.names.join(',')}`)

// 1) 최소 폭으로 줄이기
await dragSidebar(150) // 최소 200px로 막힘
tb = await toolbar()
check('최소 폭(200px)으로 줄어듦', tb.width === 200, `${tb.width}px`)
check('좁으면: 왼쪽 4개 + 더 보기, 잘린 버튼 없음', tb.names.join(',') === '서비스 추가,그룹 추가,가져오기,브랜치,⋯' && tb.clipped === 0, `${tb.names.join(',')} / 잘림 ${tb.clipped}`)

// 2) 더 보기 메뉴에서 설정 열기
await app.click('.toolbar .tool.more')
check('더 보기 메뉴', await menuOpen())
const items = await app.evaluate(
  `[...document.querySelectorAll('.menu .item, .menu .sep')].map(e => e.classList.contains('sep') ? '|' : e.querySelector('.label').textContent + (e.disabled ? '(꺼짐)' : ''))`,
)
check(
  '메뉴 항목: 전체 중지 | 런타임·설정·워크스페이스 설정·사이드바 자동 숨기기',
  items.join(',') === '전체 중지(꺼짐),|,런타임,설정,워크스페이스 설정,사이드바 자동 숨기기',
  items.join(','),
)
await app.shot('toolbar-01-more-menu.png')
await menuItem(`b.textContent.includes('설정') && !b.textContent.includes('워크스페이스')`)
check('메뉴에서 설정 화면 열림', await app.waitFor(app.ui(`document.querySelector('.panel h2')?.textContent === '설정'`), 3000))
check('설정 화면일 때 더 보기 버튼 강조', (await app.evaluate(`document.querySelector('.toolbar .tool.more').classList.contains('active')`)) === true)
await app.click('.toolbar .tool.more')
await menuOpen()
await menuItem(`b.textContent.includes('워크스페이스 설정')`)
check('메뉴에서 워크스페이스 설정 열림', await app.waitFor(app.ui(`!!document.querySelector('.panel') && document.body.textContent.includes('공통 환경변수')`), 3000))

// 3) 중간 폭: 들어가는 만큼 보이고 나머지만 메뉴로
await dragSidebar(240)
tb = await toolbar()
check(
  '240px: 6개 + 더 보기',
  tb.width === 240 && tb.names.join(',') === '서비스 추가,그룹 추가,가져오기,브랜치,실행 중인 서비스 모두 중지,런타임,⋯' && tb.clipped === 0,
  `${tb.width}px ${tb.names.join(',')}`,
)

// 4) 다시 넓히면 모두
await dragSidebar(320)
tb = await toolbar()
check('320px: 다시 모두', tb.names.length === all && !tb.names.includes('⋯') && tb.clipped === 0, `${tb.width}px ${tb.names.join(',')}`)

// 5) 좁은 폭으로 저장된 채 다시 켜도
await dragSidebar(200)
await app.quit()
app = await t.launch()
check('다시 켬', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 2`), 30000))
await sleep(500)
tb = await toolbar()
check('좁은 폭으로 다시 켜도 더 보기', tb.width === 200 && tb.names.at(-1) === '⋯' && tb.clipped === 0, `${tb.width}px ${tb.names.join(',')}`)
await app.shot('toolbar-02-narrow.png')

await t.finish()
