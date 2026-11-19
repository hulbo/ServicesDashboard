// 명령줄 인자 문자열 ↔ 배열 변환.
// 백슬래시는 Windows 경로에 자주 쓰이므로 이스케이프 문자로 취급하지 않는다.

/** 공백(줄바꿈 포함)으로 나누되 "..." 또는 '...'로 묶인 부분은 하나로 본다. */
export function splitArgs(input: string): string[] {
  const out: string[] = []
  let cur = ''
  let quote: '"' | "'" | null = null
  let has = false
  for (const ch of input) {
    if (quote) {
      if (ch === quote) quote = null
      else cur += ch
      continue
    }
    if (ch === '"' || ch === "'") {
      quote = ch
      has = true
    } else if (/\s/.test(ch)) {
      if (has) {
        out.push(cur)
        cur = ''
        has = false
      }
    } else {
      cur += ch
      has = true
    }
  }
  if (has) out.push(cur)
  return out
}

/** splitArgs의 역변환. 공백·따옴표가 있으면 따옴표로 감싼다. */
export function joinArgs(args: string[] | undefined): string {
  if (!args?.length) return ''
  return args
    .map((a) => {
      if (a !== '' && !/[\s"']/.test(a)) return a
      return a.includes('"') ? `'${a}'` : `"${a}"`
    })
    .join(' ')
}

/** 쉼표로 구분된 목록 (프로파일 등) */
export function splitList(input: string): string[] {
  return input
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
}

export function parsePorts(input: string): number[] {
  return splitList(input.replace(/\s+/g, ','))
    .map((s) => Number(s))
    .filter((n) => Number.isInteger(n) && n > 0 && n < 65536)
}

/** 줄 단위 목록 (env 파일 등) */
export function splitLines(input: string): string[] {
  return input
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean)
}
