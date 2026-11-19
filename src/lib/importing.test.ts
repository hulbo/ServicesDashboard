import { describe, expect, it } from 'vitest'
import { applyImport, initialItems, mergeImported, uniqueName, withVar } from './importing'
import type { Candidate, ScanResult, Service, Workspace } from './types'

const root = 'C:/Project/shop'

function cand(key: string, service: Partial<Service>, extra: Partial<Candidate> = {}): Candidate {
  return {
    key,
    source: 'intellij',
    origin: '.run/x.run.xml',
    group: null,
    service: { id: '', name: 'svc', type: 'npm', source: { kind: 'intellij', root, key, origin: '.run/x.run.xml' }, ...service } as Service,
    scripts: [],
    profiles: [],
    warnings: [],
    recommended: true,
    command: null,
    commandError: null,
    ...extra,
  }
}

let seq = 0
const newId = (p: string) => `${p}-${++seq}`

describe('경로 → 변수', () => {
  it('root 아래 경로만 바꾼다 (대소문자·역슬래시 무시)', () => {
    expect(withVar('C:\\project\\SHOP\\front\\portal', root, 'SHOP')).toBe('${var:SHOP}/front/portal')
    expect(withVar('C:/Project/shop', root, 'SHOP')).toBe('${var:SHOP}')
    expect(withVar('C:/Project/shopping', root, 'SHOP')).toBe('C:/Project/shopping')
    expect(withVar(undefined, root, 'SHOP')).toBeUndefined()
  })
})

describe('applyImport', () => {
  const scan: ScanResult = {
    root,
    suggestedVar: 'SHOP',
    warnings: [],
    presets: [],
    candidates: [
      cand('a', { name: 'portal', cwd: `${root}/front/portal`, npm: { script: 'dev' } }, { group: 'npm', scripts: ['build', 'dev'] }),
      cand('b', { name: 'Api', type: 'gradle-boot', cwd: root, spring: { profiles: ['local'] } }, { group: 'server', profiles: ['local', 'prod'] }),
      cand('c', { name: 'Api', type: 'gradle-boot', cwd: root }, { group: 'server' }),
    ],
  }

  it('변수·그룹을 만들고 이름 충돌을 피한다', () => {
    const ws: Workspace = { name: 'w', groups: [{ id: 'g0', name: 'NPM' }], services: [] }
    const items = initialItems(ws, scan)
    items[1].profiles = 'prod, qa'
    items[0].script = 'build'
    const r = applyImport(ws, scan, items, 'SHOP', newId)
    expect(r.added).toHaveLength(3)
    expect(ws.variables).toEqual({ SHOP: root })
    // 이름이 같은 그룹(대소문자 무시)은 재사용, 없으면 새로 만든다
    expect(ws.groups.map((g) => g.name)).toEqual(['NPM', 'server'])
    const [portal, api, api2] = ws.services
    expect(portal.group).toBe('g0')
    expect(portal.cwd).toBe('${var:SHOP}/front/portal')
    expect(portal.npm?.script).toBe('build')
    expect(api.spring?.profiles).toEqual(['prod', 'qa'])
    expect(api2.name).toBe('Api (2)')
    expect(api.source?.key).toBe('b')
  })

  it('이미 가져온 후보는 기본 해제, 선택하면 기존 서비스를 갱신한다', () => {
    const ws: Workspace = {
      name: 'w',
      variables: { SHOP: 'c:\\project\\shop\\' },
      groups: [],
      services: [{ id: 'keep', name: '내 포털', type: 'npm', notes: '메모', ports: [3000], npm: { script: 'old' }, source: { kind: 'intellij', root, key: 'a', origin: '' } }],
    }
    const items = initialItems(ws, scan)
    expect(items[0].selected).toBe(false)
    items[0].selected = true
    items[1].selected = false
    items[2].selected = false
    const r = applyImport(ws, scan, items, null, newId)
    expect(r).toEqual({ added: [], updated: ['keep'], presets: [] })
    const s = ws.services[0]
    expect(s.id).toBe('keep')
    expect(s.name).toBe('내 포털')
    expect(s.notes).toBe('메모')
    expect(s.ports).toEqual([3000])
    expect(s.npm?.script).toBe('dev') // IntelliJ 출처는 원본 스크립트로 갱신
    expect(s.cwd).toBe('${var:SHOP}/front/portal') // 같은 값의 기존 변수를 찾아 쓴다
  })
})

describe('프리셋 가져오기', () => {
  it('후보 키를 새 서비스 ID로 바꿔 프리셋을 만든다', () => {
    const scan: ScanResult = {
      root,
      suggestedVar: 'SHOP',
      warnings: [],
      candidates: [cand('a', { name: 'web' }), cand('b', { name: 'api' })],
      presets: [{ key: 'p', name: '전체', origin: '', members: ['a', 'b', 'zz'], missing: [] }],
    }
    const ws: Workspace = { name: 'w', groups: [], services: [] }
    const items = initialItems(ws, scan)
    items[1].selected = false
    const r = applyImport(ws, scan, items, null, newId, [{ preset: scan.presets[0], selected: true }])
    expect(r.presets).toEqual(['전체'])
    expect(ws.presets?.[0].services).toEqual([ws.services[0].id]) // 가져온 것만
  })
})

describe('mergeImported', () => {
  it('스캐너 출처는 사용자가 고른 스크립트·프로파일을 유지한다', () => {
    const existing: Service = { id: 'x', name: 'n', type: 'npm', npm: { script: 'serve' }, spring: { profiles: ['qa'] } }
    const fresh: Service = { id: '', name: 'm', type: 'npm', npm: { script: 'dev' }, source: { kind: 'npm', root, key: 'k', origin: '' } }
    const m = mergeImported(existing, fresh)
    expect(m.npm?.script).toBe('serve')
    expect(m.spring?.profiles).toEqual(['qa'])
    expect(m.source?.kind).toBe('npm')
  })

  it('uniqueName', () => {
    expect(uniqueName('a', new Set(['a', 'a (2)']))).toBe('a (3)')
    expect(uniqueName('b', new Set(['a']))).toBe('b')
  })
})
