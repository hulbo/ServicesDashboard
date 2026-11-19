import { describe, expect, it } from 'vitest'
import { buildRows, compileQuery, decorate, levelCounts, LEVEL_ERROR, LEVEL_WARN, rowLineCount, viewText, type ViewOptions } from './console-view'
import { LogStore } from './logstore'

function store(texts: string[]) {
  const s = new LogStore()
  s.append(texts.map((text, i) => ({ seq: i + 1, ts: 0, stream: 0, text })))
  return s
}

const base: ViewOptions = { query: null, filterMode: false, levels: new Set(), foldFrames: true, expanded: new Set() }

const trace = [
  '2026-11-19 INFO  started',
  '2026-11-19 ERROR 요청 처리 실패',
  'java.lang.IllegalStateException: 샘플',
  '\tat a.B.c(B.java:1)',
  '\tat a.B.d(B.java:2)',
  '\tat a.B.e(B.java:3)',
  '\tat a.B.f(B.java:4)',
  '\tat a.B.g(B.java:5)',
  'Caused by: java.io.IOException: x',
  '\t... 12 more',
  '2026-11-19 WARN  느림',
  'done',
]

describe('레벨', () => {
  it('ERROR/WARN과 스택트레이스 상속', () => {
    const s = store(trace)
    expect(s.lines.map((l) => l.level)).toEqual([0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2, 0])
    expect(s.lines.filter((l) => l.frame).length).toBe(6)
    expect(levelCounts(s.lines)).toEqual({ error: 3, warn: 1 })
  })
})

describe('안내 줄', () => {
  it('실행 명령에 ERROR가 있어도 세지 않고, ✕ 안내만 오류', () => {
    const s = new LogStore()
    s.append([
      { seq: 1, ts: 0, stream: 2, text: "▶ powershell -Command Write-Output 'ERROR x'" },
      { seq: 2, ts: 0, stream: 2, text: '✕ 비정상 종료 (종료 코드 1)' },
    ])
    expect(s.lines.map((l) => l.level)).toEqual([0, 3])
  })
})

describe('buildRows', () => {
  it('긴 스택트레이스는 앞 2줄만 남기고 접는다', () => {
    const s = store(trace)
    const { rows } = buildRows(s.lines, base)
    const kinds = rows.map((r) => (r.kind === 'fold' ? `fold:${r.count}` : r.line.plain.trim().slice(0, 12)))
    expect(kinds).toEqual(['2026-11-19 I', '2026-11-19 E', 'java.lang.Il', 'at a.B.c(B.j', 'at a.B.d(B.j', 'fold:3', 'Caused by: j', '... 12 more', '2026-11-19 W', 'done'])
    const fold = rows.find((r) => r.kind === 'fold')!
    const expanded = buildRows(s.lines, { ...base, expanded: new Set([fold.kind === 'fold' ? fold.key : 0]) })
    expect(expanded.rows.length).toBe(trace.length)
  })

  it('레벨 필터는 스택트레이스를 함께 보여 준다', () => {
    const s = store(trace)
    const { rows } = buildRows(s.lines, { ...base, levels: new Set([LEVEL_ERROR]) })
    expect(rows).toHaveLength(9)
    expect(rows.every((r) => r.kind === 'line')).toBe(true)
    const warn = buildRows(s.lines, { ...base, levels: new Set([LEVEL_WARN]) })
    expect(warn.rows).toHaveLength(1)
  })

  it('검색: 일치 줄 위치, 필터 모드, 대소문자·정규식', () => {
    const s = store(trace)
    const q = compileQuery('b.java', false, false) as RegExp
    const r = buildRows(s.lines, { ...base, query: q })
    expect(r.matches).toHaveLength(5) // 검색 중에는 접지 않는다
    const f = buildRows(s.lines, { ...base, query: q, filterMode: true })
    expect(f.rows).toHaveLength(5)
    expect(compileQuery('B.java', false, true)).toBeInstanceOf(RegExp)
    expect(buildRows(s.lines, { ...base, query: compileQuery('b.java', false, true) as RegExp }).matches).toHaveLength(0)
    expect(compileQuery('([', true, false)).toBe('invalid')
    expect(compileQuery('', false, false)).toBeNull()
    const re = compileQuery('B\.(f|g)', true, true) as RegExp
    expect(buildRows(s.lines, { ...base, query: re }).matches).toHaveLength(2)
  })
})

describe('전체 선택 복사 (Ctrl+A)', () => {
  it('접힌 스택트레이스까지 모든 줄을 복사한다', () => {
    const s = store(trace)
    const r = viewText(s.lines, base)
    expect(r.count).toBe(trace.length)
    expect(r.text).toBe(trace.join('\n'))
    expect(rowLineCount(buildRows(s.lines, base).rows)).toBe(trace.length)
  })

  it('레벨·검색 필터를 걸었으면 보이는 줄만, 시각 표시를 켰으면 시각도', () => {
    const s = store(trace)
    expect(viewText(s.lines, { ...base, levels: new Set([LEVEL_WARN]) }).text).toBe('2026-11-19 WARN  느림')
    const q = compileQuery('B.java:[12]', true, false) as RegExp
    expect(viewText(s.lines, { ...base, query: q, filterMode: true }).count).toBe(2)
    expect(viewText(s.lines, { ...base, query: q }).count).toBe(trace.length) // 필터 모드가 아니면 전부
    expect(viewText(store(['a']).lines, base, () => '12:00:00.000').text).toBe('12:00:00.000 a')
  })
})

describe('decorate', () => {
  it('색상 조각 위에 검색어와 링크를 입힌다', () => {
    const s = store(['\x1b[32mINFO\x1b[0m open http://0.0.0.0:5173/ now'])
    const line = s.lines[0]
    const pieces = decorate(line.segments, line.plain, compileQuery('open', false, false) as RegExp)
    expect(pieces.map((p) => p.text).join('')).toBe(line.plain)
    expect(pieces[0]).toEqual({ text: 'INFO', style: 'color:var(--ansi-2)' })
    expect(pieces.find((p) => p.match)?.text).toBe('open')
    expect(pieces.find((p) => p.href)).toMatchObject({ text: 'http://0.0.0.0:5173/', href: 'http://localhost:5173/' })
  })

  it('꾸밀 것이 없으면 그대로', () => {
    const s = store(['plain'])
    expect(decorate(s.lines[0].segments, s.lines[0].plain, null)).toEqual([{ text: 'plain', style: '' }])
  })
})
