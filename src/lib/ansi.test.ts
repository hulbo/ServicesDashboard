import { describe, expect, it } from 'vitest'
import { parseAnsi } from './ansi'
import { LogStore } from './logstore'

describe('parseAnsi', () => {
  it('SGR 색상과 굵게를 스타일로 바꾼다', () => {
    const r = parseAnsi('\x1b[32m\x1b[1mVITE\x1b[22m v5\x1b[39m done')
    expect(r.plain).toBe('VITE v5 done')
    expect(r.segments[0]).toEqual({ text: 'VITE', style: 'color:var(--ansi-2);font-weight:700' })
    expect(r.segments[1]).toEqual({ text: ' v5', style: 'color:var(--ansi-2)' })
    expect(r.segments[2]).toEqual({ text: ' done', style: '' })
  })

  it('256색과 트루컬러', () => {
    expect(parseAnsi('\x1b[38;5;196mA').segments[0].style).toBe('color:rgb(255,0,0)')
    expect(parseAnsi('\x1b[48;2;1;2;3mB').segments[0].style).toBe('background:rgb(1,2,3)')
    expect(parseAnsi('\x1b[38;5;244mC').segments[0].style).toBe('color:rgb(128,128,128)')
  })

  it('커서 이동·OSC·제어 문자는 버린다', () => {
    const r = parseAnsi('\x1b[2K\x1b[1Gprogress\x1b]0;title\x07 ok\x08\t!')
    expect(r.plain).toBe('progress ok\t!')
  })

  it('상태가 다음 줄로 이어진다', () => {
    const a = parseAnsi('\x1b[31merror start')
    const b = parseAnsi('continued\x1b[0m plain', a.state)
    expect(b.segments[0].style).toBe('color:var(--ansi-1)')
    expect(b.segments[1].style).toBe('')
  })

  it('반전은 전경·배경을 바꾼다', () => {
    expect(parseAnsi('\x1b[7mX').segments[0].style).toBe('color:var(--console-bg);background:var(--console-fg)')
  })
})

describe('LogStore', () => {
  const line = (seq: number, text = `line ${seq}`) => ({ seq, ts: 0, stream: 0, text })

  it('중복 seq를 무시하고 용량을 넘으면 앞을 자른다', () => {
    const s = new LogStore(100)
    s.append(Array.from({ length: 150 }, (_, i) => line(i + 1)))
    s.append([line(150), line(151)])
    expect(s.lines.length).toBe(100)
    expect(s.lines[0].seq).toBe(52)
    expect(s.lastSeq).toBe(151)
    expect(s.removedTotal).toBe(51)
  })

  it('reset은 스냅샷 뒤에 받은 줄을 유지한다', () => {
    const s = new LogStore()
    s.append([line(5), line(6)])
    s.reset([line(1), line(2), line(3), line(4), line(5)])
    expect(s.lines.map((l) => l.seq)).toEqual([1, 2, 3, 4, 5, 6])
  })
})
