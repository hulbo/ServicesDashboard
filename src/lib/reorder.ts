/**
 * 목록 순서 바꾸기 (환경변수·변수 편집기).
 *
 * `to`는 원래 배열 기준의 "끼워 넣을 자리"다: 0이면 맨 앞, `list.length`면 맨 뒤.
 * 끌어서 놓을 때 "i번째 행 위/아래"를 그대로 i / i+1로 넘기면 된다.
 * 순서가 바뀌었으면 true.
 */
export function moveTo<T>(list: T[], from: number, to: number): boolean {
  if (from < 0 || from >= list.length) return false
  let at = Math.max(0, Math.min(list.length, to))
  if (at > from) at -= 1
  if (at === from) return false
  const [item] = list.splice(from, 1)
  list.splice(at, 0, item)
  return true
}

/** 한 칸 위(-1)·아래(+1)로. 끝에서는 그대로. 새 위치를 돌려준다. */
export function step<T>(list: T[], index: number, dir: -1 | 1): number {
  const at = index + dir
  if (at < 0 || at >= list.length) return index
  moveTo(list, index, dir > 0 ? at + 1 : at)
  return at
}

/**
 * 이름순 정렬: 대소문자 무시, 숫자는 크기순(VAR_2 < VAR_10), 이름이 빈 행은 맨 뒤.
 * 이미 정렬돼 있으면 false.
 */
export function sortByName<T>(list: T[], name: (item: T) => string): boolean {
  const sorted = [...list].sort((a, b) => compareNames(name(a), name(b)))
  if (sorted.every((x, i) => x === list[i])) return false
  list.splice(0, list.length, ...sorted)
  return true
}

export function isSortedByName(names: string[]): boolean {
  return names.every((n, i) => i === 0 || compareNames(names[i - 1], n) <= 0)
}

function compareNames(a: string, b: string): number {
  const ka = a.trim()
  const kb = b.trim()
  if (!ka || !kb) return ka ? -1 : kb ? 1 : 0
  return ka.localeCompare(kb, 'en', { sensitivity: 'base', numeric: true })
}
