// 환경변수 순서 바꾸기: 그룹·서비스 편집기의 끌어서 놓기, Alt+↑/↓, 이름순 정렬, 저장한 파일의 키 순서 유지
import { createSuite, sleep } from './lib.mjs'

const t = createSuite('reorder')
t.writeSettings()
t.writeWorkspace({
  groups: [{ id: 'data', name: 'data', env: { B_URL: 'jdbc:test://b', A_PASSWORD: 'pw-a', C_USER: 'c', A_USER: 'a' } }],
  services: [{ id: 'svc', name: 'svc', group: 'data', type: 'shell', env: { Y: '2', X: '1' }, shell: { command: 'echo hi', shell: 'cmd' } }],
})

const app = await t.launch()
const { check } = t
let dragData = null
app.on('Input.dragIntercepted', (p) => (dragData = p.data))

const order = () => app.evaluate(`[...document.querySelectorAll('.kv input.key')].map(i => i.value).join(',')`)
const diskOrder = (pick) => Object.keys(pick(t.readWorkspace()) ?? {}).join(',')
const saveBtn = `[...document.querySelectorAll('.save-row .btn')].find(b => b.textContent.includes('저장'))`
const saveDisabled = () => app.evaluate(`${saveBtn}.disabled`)

/** 실제 마우스로 손잡이를 눌러 끌고, 목표 행의 위(0.2)·아래(0.8)에 놓는다. 놓기 직전의 표시를 돌려준다. */
async function drag(from, to, part) {
  await app.send('Input.setInterceptDrags', { enabled: true })
  dragData = null
  const g = await app.rect('.kv .grip', from)
  const r = await app.rect('.kv .row', to)
  const sx = g.l + g.w / 2
  const sy = g.t + g.h / 2
  const tx = r.l + 120
  const ty = r.t + r.h * part
  // 새로 뜬 창은 첫 끌기를 놓치는 일이 있어 몇 번 다시 시도한다 (시험 도구 문제, 앱 동작과 무관)
  for (let attempt = 0; attempt < 4 && !dragData; attempt++) {
    await app.mouse('mouseMoved', sx, sy)
    await app.mouse('mousePressed', sx, sy, 1)
    for (let k = 1; k <= 8 && !dragData; k++) {
      await app.mouse('mouseMoved', sx + ((tx - sx) * k) / 8, sy + ((ty - sy) * k) / 8, 1)
      await sleep(40)
    }
    if (!(await app.waitFor(async () => !!dragData, 1500))) {
      await app.mouse('mouseReleased', tx, ty)
      await sleep(300)
    }
  }
  if (!dragData) throw new Error('끌기가 시작되지 않음')
  await app.send('Input.dispatchDragEvent', { type: 'dragEnter', x: tx, y: ty, data: dragData })
  await app.send('Input.dispatchDragEvent', { type: 'dragOver', x: tx, y: ty, data: dragData })
  await sleep(100)
  const marks = await app.evaluate(
    `[...document.querySelectorAll('.kv .row')].map((r, i) => (r.classList.contains('drop-before') ? 'B' + i : '') + (r.classList.contains('drop-after') ? 'A' + i : '') + (r.classList.contains('dragging') ? 'D' + i : '')).filter(Boolean).join(' ')`,
  )
  if (part > 0.5 && to === 3) await app.shot('reorder-01-dragging.png')
  await app.send('Input.dispatchDragEvent', { type: 'drop', x: tx, y: ty, data: dragData })
  await app.mouse('mouseReleased', tx, ty)
  await app.send('Input.setInterceptDrags', { enabled: false })
  await sleep(150)
  return marks
}

check('앱 시작', await app.waitFor(app.ui(`document.querySelectorAll('.list .row').length >= 2`), 30000))
await app.clickRow('data')
check('그룹 환경변수: 파일 순서대로', await app.waitFor(async () => (await order()) === 'B_URL,A_PASSWORD,C_USER,A_USER', 5000), await order())
check('행마다 손잡이', (await app.evaluate(`document.querySelectorAll('.kv .grip:not(:disabled)').length`)) === 4)

// 1) 제자리에 놓으면 아무것도 바뀌지 않음
let marks = await drag(1, 1, 0.2)
check('제자리: 놓을 자리 표시 없음', marks === 'D1', marks)
check('제자리: 순서·저장 버튼 그대로', (await order()) === 'B_URL,A_PASSWORD,C_USER,A_USER' && (await saveDisabled()), await order())

// 2) 마지막 행을 맨 위로 (비밀값 보기 상태가 행을 따라가는지도)
await app.evaluate(`document.querySelectorAll('.kv .row')[1].querySelector('button[title="값 보기"]').click(); true`)
marks = await drag(3, 0, 0.2)
check('맨 위로: 첫 행 위에 표시', marks === 'B0 D3', marks)
check('맨 위로 옮김', (await order()) === 'A_USER,B_URL,A_PASSWORD,C_USER', await order())
check('끌기 표시 정리', (await app.evaluate(`document.querySelectorAll('.kv .row.dragging, .kv .row.drop-before, .kv .row.drop-after').length`)) === 0)
check('보기 상태가 행을 따라감', (await app.evaluate(`document.querySelectorAll('.kv .row')[2].querySelector('input.val').type`)) === 'text')
check('저장 버튼 켜짐', !(await saveDisabled()))

// 3) 첫 행을 맨 아래로
marks = await drag(0, 3, 0.8)
check('맨 아래로: 마지막 행 아래에 표시', marks === 'D0 A3', marks)
check('맨 아래로 옮김', (await order()) === 'B_URL,A_PASSWORD,C_USER,A_USER', await order())

// 4) 키보드: 이름 칸에서 Alt+↑, 손잡이에서 ↓
await app.evaluate(`document.querySelectorAll('.kv input.key')[3].focus(); true`)
await app.press('Alt+ArrowUp')
check('Alt+↑ 한 칸 위로', (await order()) === 'B_URL,A_PASSWORD,A_USER,C_USER', await order())
check('Alt+↑ 후에도 같은 칸에 포커스', (await app.evaluate(`document.activeElement?.value`)) === 'A_USER')
await app.press('Alt+ArrowUp')
await app.press('Alt+ArrowUp')
await app.press('Alt+ArrowUp') // 맨 위에서 더 올리면 그대로
check('맨 위에서 Alt+↑는 그대로', (await order()) === 'A_USER,B_URL,A_PASSWORD,C_USER', await order())
await app.evaluate(`document.querySelectorAll('.kv .grip')[0].focus(); true`)
await app.press('ArrowDown')
check('손잡이에서 ↓', (await order()) === 'B_URL,A_USER,A_PASSWORD,C_USER', await order())
check('손잡이 포커스 유지', (await app.evaluate(`document.activeElement === document.querySelectorAll('.kv .grip')[1]`)) === true)

// 5) 저장 → 앱 상태·파일 모두 같은 순서
await app.evaluate(`${saveBtn}.click(); true`)
check('저장 완료', await app.waitFor(saveDisabled, 5000))
const ws = (await app.invoke('workspace_get')).workspace
check('앱 상태 순서', Object.keys(ws.groups[0].env).join(',') === 'B_URL,A_USER,A_PASSWORD,C_USER', Object.keys(ws.groups[0].env).join(','))
check('파일에 저장된 순서', diskOrder((w) => w.groups[0].env) === 'B_URL,A_USER,A_PASSWORD,C_USER', diskOrder((w) => w.groups[0].env))
check('값은 그대로', ws.groups[0].env.A_PASSWORD === 'pw-a' && ws.groups[0].env.B_URL === 'jdbc:test://b')

// 6) 이름순 정렬
const sortBtn = `[...document.querySelectorAll('.kv .actions button')].find(b => b.textContent.includes('이름순'))`
check('이름순 정렬 버튼 켜짐', (await app.evaluate(`${sortBtn}.disabled`)) === false)
await app.evaluate(`${sortBtn}.click(); true`)
check('이름순 정렬', (await order()) === 'A_PASSWORD,A_USER,B_URL,C_USER', await order())
check('정렬 후 버튼 꺼짐', (await app.evaluate(`${sortBtn}.disabled`)) === true)
await app.shot('reorder-02-sorted.png')
await app.evaluate(`${saveBtn}.click(); true`)
check(
  '정렬 저장 → 파일 순서',
  await app.waitFor(async () => diskOrder((w) => w.groups[0].env) === 'A_PASSWORD,A_USER,B_URL,C_USER', 5000),
  diskOrder((w) => w.groups[0].env),
)

// 7) 서비스 편집 화면의 환경변수도 같은 기능
await app.clickRow('svc')
await app.waitFor(app.ui(`[...document.querySelectorAll('.tabs button')].some(b => b.textContent.trim() === '설정')`), 5000)
await app.evaluate(`[...document.querySelectorAll('.tabs button')].find(b => b.textContent.trim() === '설정').click(); true`)
check('서비스 환경변수 순서', await app.waitFor(async () => (await order()) === 'Y,X', 5000), await order())
await app.evaluate(`document.querySelectorAll('.kv input.key')[0].focus(); true`)
await app.press('Alt+ArrowDown')
check('서비스: Alt+↓', (await order()) === 'X,Y', await order())
check('서비스: 저장하지 않은 변경 표시', await app.waitFor(app.ui(`!!document.querySelector('.dirty')`), 3000))
await app.press('Ctrl+S')
check('서비스 저장 → 파일 순서', await app.waitFor(async () => diskOrder((w) => w.services[0].env) === 'X,Y', 5000), diskOrder((w) => w.services[0].env))

await t.finish()
