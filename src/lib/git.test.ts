import { describe, expect, it } from 'vitest'
import { detectHeadChanges, headLabel, matchBranch } from './git'
import type { GitRepo } from './types'

const repo = (root: string, branch: string | null, commit: string | null = null): GitRepo => ({ root, name: root, branch, commit, services: [] })

describe('브랜치 변경 감지', () => {
  it('처음 본 저장소는 알리지 않고, 바뀐 것만 알린다', () => {
    const seen = new Map<string, string>()
    expect(detectHeadChanges(seen, [repo('a', 'main'), repo('b', 'dev')])).toEqual([])
    expect(detectHeadChanges(seen, [repo('a', 'main'), repo('b', 'dev')])).toEqual([])
    const ch = detectHeadChanges(seen, [repo('a', 'feature/x'), repo('b', 'dev')])
    expect(ch.map((c) => [c.repo.root, c.from, headLabel(c.repo)])).toEqual([['a', 'main', 'feature/x']])
    // 이미 기록이 바뀌었으므로 다시 알리지 않는다
    expect(detectHeadChanges(seen, [repo('a', 'feature/x')])).toEqual([])
  })

  it('이 앱이 전환 중인 저장소는 기록만 바꾼다', () => {
    const seen = new Map([['a', 'main']])
    expect(detectHeadChanges(seen, [repo('a', 'dev')], new Set(['a']))).toEqual([])
    expect(seen.get('a')).toBe('dev')
  })

  it('브랜치가 아닌 커밋(detached)도 구분한다', () => {
    const seen = new Map([['a', 'main']])
    const ch = detectHeadChanges(seen, [repo('a', null, '3f2a9c1d')])
    expect(headLabel(ch[0].repo)).toBe('3f2a9c1d (브랜치 아님)')
  })

  it('브랜치 검색', () => {
    expect(matchBranch('feature/LOGIN-page', 'login feat')).toBe(true)
    expect(matchBranch('feature/login', 'logout')).toBe(false)
    expect(matchBranch('main', '')).toBe(true)
  })
})
