import { describe, expect, it } from 'vitest'
import { badgeTitle, serviceBadge } from './rail'

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
