// ANSI 이스케이프 시퀀스(SGR) 파서.
// 색상·굵게 등은 인라인 스타일 문자열로 바꾸고, 커서 이동 같은 나머지 시퀀스는 버린다.

export interface AnsiState {
  fg?: string
  bg?: string
  bold?: boolean
  dim?: boolean
  italic?: boolean
  underline?: boolean
  strike?: boolean
  inverse?: boolean
}

export interface Segment {
  text: string
  /** 인라인 CSS. 스타일이 없으면 빈 문자열 */
  style: string
}

const ESC = '\x1b'

function basic(n: number): string {
  return `var(--ansi-${n})`
}

function xterm256(n: number): string {
  if (n < 16) return basic(n)
  if (n >= 232) {
    const v = 8 + (n - 232) * 10
    return `rgb(${v},${v},${v})`
  }
  const i = n - 16
  const conv = (c: number) => (c === 0 ? 0 : 55 + c * 40)
  return `rgb(${conv(Math.floor(i / 36))},${conv(Math.floor(i / 6) % 6)},${conv(i % 6)})`
}

function applySgr(params: number[], s: AnsiState): AnsiState {
  const st = { ...s }
  if (params.length === 0) params = [0]
  for (let i = 0; i < params.length; i++) {
    const p = params[i]
    if (p === 0) {
      for (const k of Object.keys(st)) delete (st as Record<string, unknown>)[k]
    } else if (p === 1) st.bold = true
    else if (p === 2) st.dim = true
    else if (p === 3) st.italic = true
    else if (p === 4) st.underline = true
    else if (p === 7) st.inverse = true
    else if (p === 9) st.strike = true
    else if (p === 21 || p === 22) {
      st.bold = false
      st.dim = false
    } else if (p === 23) st.italic = false
    else if (p === 24) st.underline = false
    else if (p === 27) st.inverse = false
    else if (p === 29) st.strike = false
    else if (p >= 30 && p <= 37) st.fg = basic(p - 30)
    else if (p === 39) delete st.fg
    else if (p >= 40 && p <= 47) st.bg = basic(p - 40)
    else if (p === 49) delete st.bg
    else if (p >= 90 && p <= 97) st.fg = basic(p - 90 + 8)
    else if (p >= 100 && p <= 107) st.bg = basic(p - 100 + 8)
    else if (p === 38 || p === 48) {
      const key = p === 38 ? 'fg' : 'bg'
      if (params[i + 1] === 5 && params[i + 2] !== undefined) {
        st[key] = xterm256(params[i + 2])
        i += 2
      } else if (params[i + 1] === 2 && params[i + 4] !== undefined) {
        st[key] = `rgb(${params[i + 2]},${params[i + 3]},${params[i + 4]})`
        i += 4
      }
    }
  }
  return st
}

function toCss(s: AnsiState): string {
  let fg = s.fg
  let bg = s.bg
  if (s.inverse) {
    fg = s.bg ?? 'var(--console-bg)'
    bg = s.fg ?? 'var(--console-fg)'
  }
  const parts: string[] = []
  if (fg) parts.push(`color:${fg}`)
  if (bg) parts.push(`background:${bg}`)
  if (s.bold) parts.push('font-weight:700')
  if (s.dim) parts.push('opacity:.65')
  if (s.italic) parts.push('font-style:italic')
  const deco = [s.underline && 'underline', s.strike && 'line-through'].filter(Boolean)
  if (deco.length) parts.push(`text-decoration:${deco.join(' ')}`)
  return parts.join(';')
}

/** 제어 문자(탭 제외)를 지운다. */
function stripControls(text: string): string {
  // eslint-disable-next-line no-control-regex
  return text.replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '')
}

export function parseAnsi(text: string, state: AnsiState = {}): { segments: Segment[]; plain: string; state: AnsiState } {
  const segments: Segment[] = []
  let cur = state
  let plain = ''
  let i = 0
  let buf = ''
  const flush = () => {
    if (!buf) return
    const t = stripControls(buf)
    if (t) {
      segments.push({ text: t, style: toCss(cur) })
      plain += t
    }
    buf = ''
  }
  while (i < text.length) {
    const ch = text[i]
    if (ch !== ESC) {
      buf += ch
      i++
      continue
    }
    const next = text[i + 1]
    if (next === '[') {
      // CSI: ESC [ 파라미터 중간바이트 최종바이트(@~)
      let j = i + 2
      while (j < text.length && /[0-?]/.test(text[j])) j++
      while (j < text.length && /[ -/]/.test(text[j])) j++
      if (j >= text.length) {
        i = text.length
        break
      }
      const final = text[j]
      if (final === 'm') {
        flush()
        const raw = text.slice(i + 2, j)
        const params = raw === '' ? [] : raw.split(/[;:]/).map((x) => (x === '' ? 0 : Number(x) || 0))
        cur = applySgr(params, cur)
      }
      i = j + 1
    } else if (next === ']') {
      // OSC: BEL 또는 ESC \ 로 끝난다 (터미널 제목, 하이퍼링크 등)
      let j = i + 2
      while (j < text.length && text[j] !== '\x07' && !(text[j] === ESC && text[j + 1] === '\\')) j++
      i = text[j] === '\x07' ? j + 1 : j + 2
    } else {
      i += 2
    }
  }
  flush()
  return { segments, plain, state: cur }
}
