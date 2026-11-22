/**
 * 사이드바를 자동 숨김으로 두었을 때 왼쪽 기둥(레일)에 보이는 서비스 아이콘의 글자.
 *
 * - 첫 단어가 한글이면 앞 두 글자: "수혜이력 이관" → "수혜"
 * - 단어가 둘 이상이면 앞 두 단어의 첫 글자: "PortalApplication (local)" → "PA", "isbm-paApi" → "IP"
 * - 한 단어면 앞 두 글자: "portal" → "Po"
 */
export function serviceBadge(name: string): string {
  const n = name.trim()
  if (!n) return '?'
  const words = n
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .split(/[\s\-_.()[\]/:,]+/)
    .filter(Boolean)
  const first = words[0] ?? n
  if (/^[가-힣]/.test(first)) return [...first].slice(0, 2).join('')
  if (words.length >= 2) return ([...words[0]][0] + [...words[1]][0]).toUpperCase()
  const chars = [...first].slice(0, 2)
  return chars[0].toUpperCase() + (chars[1] ?? '')
}

/** 기둥 아이콘에 마우스를 올렸을 때 보이는 설명 */
export function badgeTitle(name: string, stateLabel: string, ports: number[]): string {
  return [name, stateLabel, ...ports.slice(0, 3).map((p) => `:${p}`)].join(' · ')
}

/** 점(마지막 마우스 위치)이 사각형(getBoundingClientRect) 안에 있는지. 오른쪽·아래 끝은 바깥으로 본다. */
export function pointInRect(
  p: { x: number; y: number } | null,
  r: { left: number; top: number; right: number; bottom: number },
): boolean {
  return !!p && p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}
