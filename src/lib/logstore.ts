// 서비스별 콘솔 로그 저장소 (화면용으로 ANSI를 미리 해석해 둔다).
// 수만 줄을 다루므로 Svelte 반응형 상태가 아닌 일반 객체로 두고,
// 변경 알림은 app.logTick 카운터로 한다.

import { parseAnsi, type AnsiState, type Segment } from './ansi'
import { LEVEL_ERROR, LEVEL_NONE, isFrame, lineLevel, type Level } from './console-view'
import type { LogLine } from './types'

export const DEFAULT_CAPACITY = 20_000

export interface ViewLine {
  seq: number
  ts: number
  stream: number
  segments: Segment[]
  plain: string
  /** 로그 레벨 (스택트레이스는 앞 줄 레벨을 물려받음) */
  level: Level
  /** 스택 프레임 줄 (`at ...`) */
  frame: boolean
}

export class LogStore {
  lines: ViewLine[] = []
  lastSeq = 0
  /** 용량 초과로 앞에서 잘려 나간 줄의 누적 수 (스크롤 위치 보정용) */
  removedTotal = 0
  private ansi: AnsiState[] = [{}, {}, {}]

  constructor(public capacity = DEFAULT_CAPACITY) {}

  append(lines: LogLine[]): number {
    let added = 0
    for (const l of lines) {
      if (l.seq <= this.lastSeq) continue
      const stream = l.stream in this.ansi ? l.stream : 0
      const parsed = parseAnsi(l.text, this.ansi[stream])
      this.ansi[stream] = parsed.state
      const prev = this.lines.length ? this.lines[this.lines.length - 1].level : LEVEL_NONE
      this.lines.push({
        seq: l.seq,
        ts: l.ts,
        stream: l.stream,
        segments: parsed.segments,
        plain: parsed.plain,
        // 대시보드 안내 줄은 실행 명령 문자열에 ERROR 같은 단어가 있어도 레벨로 세지 않는다 (실패 안내 ✕만 오류)
        level: l.stream === 2 ? (parsed.plain.startsWith('✕') ? LEVEL_ERROR : LEVEL_NONE) : lineLevel(parsed.plain, prev),
        frame: isFrame(parsed.plain),
      })
      this.lastSeq = l.seq
      added++
    }
    const excess = this.lines.length - this.capacity
    if (excess > 0) {
      this.lines.splice(0, excess)
      this.removedTotal += excess
    }
    return added
  }

  /** 백엔드 버퍼 스냅샷으로 다시 채운다. 이미 받은 더 최신 줄은 유지한다. */
  reset(snapshot: LogLine[]) {
    const newer = this.lines.filter((l) => snapshot.length === 0 || l.seq > snapshot[snapshot.length - 1].seq)
    this.lines = []
    this.lastSeq = 0
    this.ansi = [{}, {}, {}]
    this.append(snapshot)
    for (const l of newer) {
      if (l.seq > this.lastSeq) {
        this.lines.push(l)
        this.lastSeq = l.seq
      }
    }
  }

  clear() {
    this.removedTotal += this.lines.length
    this.lines = []
    this.ansi = [{}, {}, {}]
  }

  text(): string {
    return this.lines.map((l) => l.plain).join('\n')
  }
}
