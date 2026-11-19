import { describe, expect, it } from 'vitest'
import { isSortedByName, moveTo, sortByName, step } from './reorder'

describe('순서 바꾸기', () => {
  it('끼워 넣을 자리 기준으로 옮긴다', () => {
    const a = ['A', 'B', 'C', 'D']
    expect(moveTo(a, 3, 0)).toBe(true) // D를 A 위로
    expect(a).toEqual(['D', 'A', 'B', 'C'])
    expect(moveTo(a, 0, 4)).toBe(true) // D를 맨 뒤로
    expect(a).toEqual(['A', 'B', 'C', 'D'])
    expect(moveTo(a, 0, 2)).toBe(true) // A를 C 위(= B 아래)로
    expect(a).toEqual(['B', 'A', 'C', 'D'])
  })

  it('제자리에 놓으면 바뀌지 않는다', () => {
    const a = ['A', 'B', 'C']
    expect(moveTo(a, 1, 1)).toBe(false) // B를 B 위로
    expect(moveTo(a, 1, 2)).toBe(false) // B를 B 아래(= C 위)로
    expect(moveTo(a, 5, 0)).toBe(false)
    expect(a).toEqual(['A', 'B', 'C'])
  })

  it('한 칸씩 위·아래, 끝에서는 그대로', () => {
    const a = ['A', 'B', 'C']
    expect(step(a, 2, -1)).toBe(1)
    expect(a).toEqual(['A', 'C', 'B'])
    expect(step(a, 0, 1)).toBe(1)
    expect(a).toEqual(['C', 'A', 'B'])
    expect(step(a, 0, -1)).toBe(0)
    expect(step(a, 2, 1)).toBe(2)
    expect(a).toEqual(['C', 'A', 'B'])
  })

  it('이름순: 대소문자 무시, 숫자 크기순, 빈 이름은 맨 뒤', () => {
    const a = [{ k: 'DB_URL' }, { k: '' }, { k: 'api_user' }, { k: 'VAR_10' }, { k: 'VAR_2' }, { k: 'DB_PASSWORD' }]
    expect(sortByName(a, (x) => x.k)).toBe(true)
    expect(a.map((x) => x.k)).toEqual(['api_user', 'DB_PASSWORD', 'DB_URL', 'VAR_2', 'VAR_10', ''])
    expect(sortByName(a, (x) => x.k)).toBe(false)
    expect(isSortedByName(['a', 'B', 'c10'])).toBe(true)
    expect(isSortedByName(['B', 'a'])).toBe(false)
  })
})
