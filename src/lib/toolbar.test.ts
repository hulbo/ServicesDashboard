import { describe, expect, it } from 'vitest'
import { MORE_W, SEP_W, TOOL_W, toolbarFit } from './toolbar'

// 실제 도구 모음: [추가, 그룹 추가, 가져오기, 브랜치] | [전체 중지] … [런타임, 설정, 워크스페이스]
const GROUPS = [0, 0, 0, 0, 1, 2, 2, 2]
const FULL = 8 * TOOL_W + SEP_W

describe('도구 모음 폭 맞추기', () => {
  it('넓으면 모두 보인다', () => {
    expect(toolbarFit(GROUPS, FULL)).toBe(8)
    expect(toolbarFit(GROUPS, 400)).toBe(8)
  })

  it('조금이라도 모자라면 더 보기 버튼 자리를 남기고 오른쪽부터 옮긴다', () => {
    expect(toolbarFit(GROUPS, FULL - 1)).toBe(6) // 6개 + 구분선 + ⋯ = 221
    expect(6 * TOOL_W + SEP_W + MORE_W).toBeLessThanOrEqual(FULL - 1)
  })

  it('사이드바 최소 폭(200px, 여백 빼고 184px)에서는 왼쪽 묶음만', () => {
    expect(toolbarFit(GROUPS, 184)).toBe(4) // 4×30 + ⋯ 30 = 150, 전체 중지까지 넣으면 구분선 포함 191
  })

  it('아주 좁아도 0개 아래로는 안 간다', () => {
    expect(toolbarFit(GROUPS, 20)).toBe(0)
  })
})
