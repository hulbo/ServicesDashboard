import { describe, expect, it } from 'vitest'
import { SERVICE_COLORS, serviceColorMap } from './colors'
import type { Service } from './types'

const svc = (id: string, group?: string) => ({ id, name: id, type: 'shell', group, shell: { command: 'x' } }) as Service
const v = (i: number) => `var(${SERVICE_COLORS[i]})`

describe('서비스 색', () => {
  it('트리 순서(그룹 순서 → 그룹 안 순서 → 그룹 없음)대로 붙인다', () => {
    // 배열 순서는 섞여 있어도 트리에 보이는 순서를 따른다
    const ws = {
      groups: [{ id: 'npm', name: 'npm' }, { id: 'server', name: 'server' }],
      services: [svc('api', 'server'), svc('tool'), svc('web', 'npm'), svc('auth', 'server'), svc('admin', 'npm'), svc('lost', 'deleted-group')],
    }
    const m = serviceColorMap(ws)
    expect(['web', 'admin', 'api', 'auth', 'tool', 'lost'].map((id) => m.get(id))).toEqual([v(0), v(1), v(2), v(3), v(4), v(5)])
  })

  it('같은 그룹 안에서는 8개까지 겹치지 않고, 그 뒤로는 돌아가며', () => {
    const ws = { groups: [{ id: 'g', name: 'g' }], services: Array.from({ length: 10 }, (_, i) => svc(`s${i}`, 'g')) }
    const colors = [...serviceColorMap(ws).values()]
    expect(new Set(colors.slice(0, 8)).size).toBe(8)
    expect(colors[8]).toBe(colors[0])
    expect(colors[9]).toBe(colors[1])
  })
})
