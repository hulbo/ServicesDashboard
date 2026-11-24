import { describe, expect, it } from 'vitest'
import { installQuestion, progressPercent, releaseUrl, statusbarLabel } from './update'

const info = { version: '0.1.11', currentVersion: '0.1.10', notes: null }

describe('업데이트 표시', () => {
  it('진행률: 전체 크기를 알 때만, 0~100으로', () => {
    expect(progressPercent({ downloaded: 1_350_000, total: 2_700_000 })).toBe(50)
    expect(progressPercent({ downloaded: 2_700_000, total: 2_700_000 })).toBe(100)
    expect(progressPercent({ downloaded: 10, total: null })).toBeNull()
    expect(progressPercent({ downloaded: 10, total: 0 })).toBeNull()
    expect(progressPercent(undefined)).toBeNull()
  })

  it('상태 표시줄: 새 버전·받는 중·설치 준비만 보이고 나머지는 숨김', () => {
    expect(statusbarLabel({ phase: 'available', info })).toBe('v0.1.11 업데이트')
    expect(statusbarLabel({ phase: 'downloading', info, progress: { downloaded: 1, total: 4 } })).toBe('업데이트 받는 중 25%')
    expect(statusbarLabel({ phase: 'downloading', info, progress: { downloaded: 1, total: null } })).toBe('업데이트 받는 중…')
    expect(statusbarLabel({ phase: 'installing', info })).toBe('업데이트 설치 준비 중…')
    for (const phase of ['idle', 'checking', 'latest', 'error'] as const) expect(statusbarLabel({ phase })).toBeNull()
  })

  it('묻는 말: 실행 중인 서비스가 있으면 중지된다고 알림', () => {
    expect(installQuestion('0.1.11', 0)).not.toContain('중지')
    expect(installQuestion('0.1.11', 2)).toContain('실행 중인 서비스 2개는 중지됩니다.')
  })

  it('릴리스 페이지 주소', () => {
    expect(releaseUrl('0.1.11')).toBe('https://github.com/hulbo/ServicesDashboard/releases/tag/v0.1.11')
  })
})
