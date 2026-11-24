import type { UpdateInfo, UpdateProgress } from './types'

/** 앱을 켠 뒤 처음 확인할 때까지 (시작을 늦추지 않게 조금 기다린다) */
export const UPDATE_FIRST_CHECK_MS = 5_000
/** 켜 둔 동안 다시 확인하는 주기 */
export const UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000

export type UpdatePhase = 'idle' | 'checking' | 'latest' | 'available' | 'downloading' | 'installing' | 'error'

export interface UpdateStatus {
  phase: UpdatePhase
  info?: UpdateInfo
  progress?: UpdateProgress
  error?: string
}

/** 받는 중 진행률(0~100). 전체 크기를 모르면 null */
export function progressPercent(p: UpdateProgress | undefined): number | null {
  if (!p || !p.total) return null
  return Math.max(0, Math.min(100, Math.floor((p.downloaded / p.total) * 100)))
}

/** 상태 표시줄에 보여 줄 글자. 보여 줄 것이 없으면 null */
export function statusbarLabel(u: UpdateStatus): string | null {
  switch (u.phase) {
    case 'available':
      return u.info ? `v${u.info.version} 업데이트` : null
    case 'downloading': {
      const pct = progressPercent(u.progress)
      return pct === null ? '업데이트 받는 중…' : `업데이트 받는 중 ${pct}%`
    }
    case 'installing':
      return '업데이트 설치 준비 중…'
    default:
      return null
  }
}

/** 업데이트 전에 묻는 말 */
export function installQuestion(version: string, running: number): string {
  const lines = [`v${version}으로 업데이트할까요?`, '', '받은 뒤 앱이 잠시 꺼졌다가 새 버전으로 다시 켜집니다.']
  if (running > 0) lines.push(`실행 중인 서비스 ${running}개는 중지됩니다.`)
  return lines.join('\n')
}

/** 이 버전의 릴리스 페이지 */
export function releaseUrl(version: string): string {
  return `https://github.com/hulbo/ServicesDashboard/releases/tag/v${version}`
}
