import { describe, expect, it } from 'vitest'
import { badgeTitle, pointInRect, serviceBadge } from './rail'

describe('기둥 아이콘 글자', () => {
  it('한글 이름은 앞 두 글자', () => {
    expect(serviceBadge('수혜이력 이관')).toBe('수혜')
    expect(serviceBadge('운영 서버')).toBe('운영')
  })

  it('여러 단어(공백·대소문자·기호)는 앞 두 단어의 첫 글자', () => {
    expect(serviceBadge('PortalApplication (local)')).toBe('PA')
    expect(serviceBadge('isbm-paApi')).toBe('IP')
    expect(serviceBadge('dataMigHrpService.migrate_ALL')).toBe('DM')
    expect(serviceBadge('batch-worker')).toBe('BW')
    expect(serviceBadge('api qa')).toBe('AQ')
  })

  it('한 단어는 앞 두 글자, 빈 이름은 ?', () => {
    expect(serviceBadge('portal')).toBe('Po')
    expect(serviceBadge('Application')).toBe('Ap')
    expect(serviceBadge('x')).toBe('X')
    expect(serviceBadge('  ')).toBe('?')
  })

  it('설명: 이름 · 상태 · 포트', () => {
    expect(badgeTitle('portal', '실행 중', [5173])).toBe('portal · 실행 중 · :5173')
    expect(badgeTitle('batch', '시작 중', [])).toBe('batch · 시작 중')
  })
})

describe('마우스 위치가 목록 안인지', () => {
  const r = { left: 44, top: 0, right: 344, bottom: 776 }
  it('안쪽·왼쪽 위 끝은 안', () => {
    expect(pointInRect({ x: 320, y: 400 }, r)).toBe(true)
    expect(pointInRect({ x: 44, y: 0 }, r)).toBe(true)
  })
  it('바깥·오른쪽 아래 끝은 밖', () => {
    expect(pointInRect({ x: 43, y: 400 }, r)).toBe(false)
    expect(pointInRect({ x: 344, y: 400 }, r)).toBe(false)
    expect(pointInRect({ x: 100, y: 776 }, r)).toBe(false)
  })
  it('마우스가 창 밖(null)이면 밖', () => {
    expect(pointInRect(null, r)).toBe(false)
  })
})
