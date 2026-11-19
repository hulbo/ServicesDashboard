// git 브랜치 표시·변경 감지용 순수 함수

import type { GitRepo } from './types'

/** 화면에 보일 HEAD 이름 (브랜치, 아니면 커밋) */
export function headLabel(r: { branch: string | null; commit: string | null }): string {
  if (r.branch) return r.branch
  return r.commit ? `${r.commit} (브랜치 아님)` : '알 수 없음'
}

/**
 * 이전에 본 HEAD와 비교해 바뀐 저장소를 찾고, 기록을 새 값으로 바꾼다.
 * 처음 보는 저장소는 바뀐 것으로 보지 않는다. `quiet`에 든 저장소(이 앱이 전환 중인 것)는 기록만 바꾼다.
 */
export function detectHeadChanges(seen: Map<string, string>, repos: GitRepo[], quiet: Set<string> = new Set()): { repo: GitRepo; from: string }[] {
  const changed: { repo: GitRepo; from: string }[] = []
  for (const r of repos) {
    const now = headLabel(r)
    const before = seen.get(r.root)
    if (before !== undefined && before !== now && !quiet.has(r.root)) changed.push({ repo: r, from: before })
    seen.set(r.root, now)
  }
  return changed
}

/** 브랜치 이름 검색: 공백으로 나눈 낱말이 모두 들어 있으면 (대소문자 무시) */
export function matchBranch(name: string, query: string): boolean {
  const n = name.toLowerCase()
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => n.includes(w))
}
