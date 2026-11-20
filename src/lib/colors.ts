import type { Workspace } from './types'

/** 서비스 구분 색 (테마별 ANSI 색 변수). 통합 로그의 서비스 이름과 기둥 아이콘 테두리가 같은 색을 쓴다. */
export const SERVICE_COLORS = ['--ansi-12', '--ansi-10', '--ansi-13', '--ansi-14', '--ansi-11', '--ansi-9', '--ansi-6', '--ansi-5']

/**
 * 서비스마다 하나의 색을 정한다. 트리 순서(그룹 순서 → 그룹 안 서비스 순서 → 그룹 없음)대로 돌아가며 붙이므로
 * 같은 그룹 안에서는 8개까지 색이 겹치지 않고, 어느 화면에서 보든 같은 서비스는 같은 색이다.
 */
export function serviceColorMap(ws: Pick<Workspace, 'groups' | 'services'>): Map<string, string> {
  const known = new Set(ws.groups.map((g) => g.id))
  const order = [
    ...ws.groups.flatMap((g) => ws.services.filter((s) => s.group === g.id)),
    ...ws.services.filter((s) => !s.group || !known.has(s.group)),
  ]
  return new Map(order.map((s, i) => [s.id, `var(${SERVICE_COLORS[i % SERVICE_COLORS.length]})`]))
}
