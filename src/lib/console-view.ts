// 콘솔 화면용 계산: 로그 레벨, 스택트레이스 접기, 검색·필터, 검색어·링크 꾸미기.
// 화면(ConsoleView)과 분리해 단위 테스트한다.

import type { Segment } from './ansi'
import type { ViewLine } from './logstore'

export const LEVEL_NONE = 0
export const LEVEL_WARN = 2
export const LEVEL_ERROR = 3
export type Level = typeof LEVEL_NONE | typeof LEVEL_WARN | typeof LEVEL_ERROR

const ERROR_RE = /\b(ERROR|FATAL|SEVERE)\b|(Exception|Error)(:|$)|^\s*Caused by:/
const WARN_RE = /\b(WARN|WARNING)\b/
const FRAME_RE = /^\s+at\s+\S|^\s*\.\.\. \d+ (more|common frames omitted)/
const CONTINUATION_RE = /^\s*Caused by:|^\s*Suppressed:/

export function ownLevel(plain: string): Level {
  if (ERROR_RE.test(plain)) return LEVEL_ERROR
  if (WARN_RE.test(plain)) return LEVEL_WARN
  return LEVEL_NONE
}

export function isFrame(plain: string): boolean {
  return FRAME_RE.test(plain)
}

/** 스택 프레임·Caused by 줄은 앞 줄의 레벨을 물려받는다 (ERROR만 볼 때 스택트레이스도 함께 보이도록). */
export function lineLevel(plain: string, prevLevel: Level): Level {
  const own = ownLevel(plain)
  if ((isFrame(plain) || CONTINUATION_RE.test(plain)) && prevLevel > own) return prevLevel
  return own
}

export type Row =
  | { kind: 'line'; line: ViewLine }
  | { kind: 'fold'; key: number; count: number }

export interface ViewOptions {
  /** 검색어 (빈 문자열이면 검색 안 함) */
  query: RegExp | null
  /** 검색어와 맞는 줄만 보이기 */
  filterMode: boolean
  /** 비어 있으면 모든 레벨 */
  levels: Set<Level>
  foldFrames: boolean
  /** 펼친 접힘 묶음의 key (첫 숨김 줄의 seq) */
  expanded: Set<number>
}

/** 연속된 스택 프레임 중 앞의 몇 줄만 남기고 접는다 */
const KEEP_FRAMES = 2
const MIN_FOLD = 3

export function compileQuery(text: string, regex: boolean, caseSensitive: boolean): RegExp | null | 'invalid' {
  if (!text) return null
  try {
    const source = regex ? text : text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    return new RegExp(source, caseSensitive ? 'g' : 'gi')
  } catch {
    return 'invalid'
  }
}

function hits(re: RegExp, plain: string): boolean {
  re.lastIndex = 0
  return re.test(plain)
}

export function buildRows(lines: ViewLine[], opts: ViewOptions): { rows: Row[]; matches: number[] } {
  const rows: Row[] = []
  const matches: number[] = []
  // 검색 중이거나 필터를 걸면 접지 않는다 (찾은 줄이 접힌 곳에 숨지 않게)
  const fold = opts.foldFrames && !opts.query && opts.levels.size === 0
  let i = 0
  while (i < lines.length) {
    const line = lines[i]
    if (fold && line.frame) {
      let j = i
      while (j < lines.length && lines[j].frame) j++
      const run = j - i
      if (run >= MIN_FOLD + KEEP_FRAMES) {
        for (let k = i; k < i + KEEP_FRAMES; k++) rows.push({ kind: 'line', line: lines[k] })
        const key = lines[i + KEEP_FRAMES].seq
        if (opts.expanded.has(key)) {
          for (let k = i + KEEP_FRAMES; k < j; k++) rows.push({ kind: 'line', line: lines[k] })
        } else {
          rows.push({ kind: 'fold', key, count: run - KEEP_FRAMES })
        }
        i = j
        continue
      }
    }
    if (opts.levels.size && !opts.levels.has(line.level)) {
      i++
      continue
    }
    const matched = opts.query ? hits(opts.query, line.plain) : false
    if (opts.filterMode && opts.query && !matched) {
      i++
      continue
    }
    if (matched) matches.push(rows.length)
    rows.push({ kind: 'line', line })
    i++
  }
  return { rows, matches }
}

/**
 * 전체 선택(Ctrl+A) 후 복사할 글: 지금 보기 조건(레벨·검색 필터)에 맞는 줄 모두.
 * 접힌 스택트레이스도 펼쳐서 넣는다. 콘솔은 보이는 줄만 그리는 가상 스크롤이라 브라우저 선택으로는 전체를 복사할 수 없다.
 */
export function viewText(lines: ViewLine[], opts: ViewOptions, time?: (ts: number) => string): { text: string; count: number } {
  const out: string[] = []
  for (const r of buildRows(lines, { ...opts, foldFrames: false }).rows) {
    if (r.kind === 'line') out.push(time ? `${time(r.line.ts)} ${r.line.plain}` : r.line.plain)
  }
  return { text: out.join('\n'), count: out.length }
}

/** 화면 행(접힘 포함)이 나타내는 실제 줄 수 */
export function rowLineCount(rows: Row[]): number {
  let n = 0
  for (const r of rows) n += r.kind === 'line' ? 1 : r.count
  return n
}

export interface Piece {
  text: string
  style: string
  match?: boolean
  href?: string
}

const URL_RE = /https?:\/\/[^\s'"<>()[\]]+/g

/** 줄의 색상 조각에 검색어 강조와 URL 링크를 입힌다. */
export function decorate(segments: Segment[], plain: string, query: RegExp | null): Piece[] {
  type Mark = { start: number; end: number; kind: 'match' | 'link'; href?: string }
  const marks: Mark[] = []
  if (query) {
    query.lastIndex = 0
    let m: RegExpExecArray | null
    while ((m = query.exec(plain))) {
      if (m[0].length === 0) {
        query.lastIndex++
        continue
      }
      marks.push({ start: m.index, end: m.index + m[0].length, kind: 'match' })
    }
  }
  URL_RE.lastIndex = 0
  let u: RegExpExecArray | null
  while ((u = URL_RE.exec(plain))) {
    const url = u[0].replace(/[.,;:]+$/, '')
    marks.push({ start: u.index, end: u.index + url.length, kind: 'link', href: url.replace('0.0.0.0', 'localhost') })
  }
  if (!marks.length) return segments.map((s) => ({ text: s.text, style: s.style }))

  const cuts = new Set<number>([0, plain.length])
  let pos = 0
  for (const s of segments) {
    cuts.add(pos)
    pos += s.text.length
  }
  for (const m of marks) {
    cuts.add(m.start)
    cuts.add(m.end)
  }
  const points = [...cuts].filter((c) => c >= 0 && c <= plain.length).sort((a, b) => a - b)
  // 각 위치의 색상 스타일
  const styleAt: string[] = []
  pos = 0
  for (const s of segments) {
    for (let k = 0; k < s.text.length; k++) styleAt[pos + k] = s.style
    pos += s.text.length
  }
  const pieces: Piece[] = []
  for (let k = 0; k < points.length - 1; k++) {
    const a = points[k]
    const b = points[k + 1]
    if (a === b) continue
    const piece: Piece = { text: plain.slice(a, b), style: styleAt[a] ?? '' }
    for (const m of marks) {
      if (m.start <= a && b <= m.end) {
        if (m.kind === 'match') piece.match = true
        else piece.href = m.href
      }
    }
    const last = pieces[pieces.length - 1]
    if (last && last.style === piece.style && last.match === piece.match && last.href === piece.href) last.text += piece.text
    else pieces.push(piece)
  }
  return pieces
}

export function levelCounts(lines: ViewLine[]): { warn: number; error: number } {
  let warn = 0
  let error = 0
  for (const l of lines) {
    if (l.level === LEVEL_ERROR && !l.frame) error++
    else if (l.level === LEVEL_WARN && !l.frame) warn++
  }
  return { warn, error }
}
