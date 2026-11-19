// 스캔 결과(후보)를 워크스페이스에 넣는 순수 로직. 화면(ImportPanel)과 다시 가져오기에서 같이 쓴다.

import type { Candidate, Group, ScanResult, Service, Workspace } from './types'

export interface ImportItem {
  candidate: Candidate
  selected: boolean
  name: string
  group: string
  /** npm 후보: 고른 스크립트 */
  script?: string
  /** Spring 후보: 쉼표로 구분한 프로파일 */
  profiles?: string
}

export function normPath(p: string): string {
  return p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()
}

/** 경로가 root 아래면 `${var:NAME}/나머지`로 바꾼다. */
export function withVar(value: string | undefined, root: string, varName: string): string | undefined {
  if (!value) return value
  const v = value.replace(/\\/g, '/')
  const r = root.replace(/\\/g, '/').replace(/\/+$/, '')
  if (normPath(v) === normPath(r)) return `\${var:${varName}}`
  if (v.toLowerCase().startsWith(r.toLowerCase() + '/')) return `\${var:${varName}}${v.slice(r.length)}`
  return value
}

/** 서비스의 경로 필드를 root 변수 참조로 바꾼다. */
export function applyRootVar(svc: Service, root: string, varName: string): Service {
  const s = structuredClone(svc)
  s.cwd = withVar(s.cwd, root, varName)
  if (s.jar) s.jar.path = withVar(s.jar.path, root, varName) ?? s.jar.path
  if (s.node) s.node.entry = withVar(s.node.entry, root, varName) ?? s.node.entry
  if (s.envFiles) s.envFiles = s.envFiles.map((f) => withVar(f, root, varName) ?? f)
  return s
}

/** root와 같은 값을 가진 워크스페이스 변수 이름 */
export function findRootVar(ws: Workspace, root: string): string | undefined {
  return Object.entries(ws.variables ?? {}).find(([, v]) => normPath(v) === normPath(root))?.[0]
}

export function uniqueName(name: string, taken: Set<string>): string {
  if (!taken.has(name)) return name
  for (let i = 2; ; i++) {
    const n = `${name} (${i})`
    if (!taken.has(n)) return n
  }
}

/**
 * 다시 가져올 때: 실행 정의는 새 값으로, 사용자가 관리하는 값(이름·그룹·메모·포트 등)은 기존 값으로.
 * IntelliJ가 아닌 출처(스캐너)는 사용자가 고른 스크립트·프로파일도 유지한다.
 */
export function mergeImported(existing: Service, fresh: Service): Service {
  const merged: Service = {
    ...fresh,
    id: existing.id,
    name: existing.name,
    group: existing.group,
  }
  for (const key of ['notes', 'encoding', 'inject'] as const) {
    if (existing[key] !== undefined) (merged as Record<string, unknown>)[key] = existing[key]
    else delete (merged as Record<string, unknown>)[key]
  }
  if (existing.ports?.length) merged.ports = existing.ports
  if (fresh.source?.kind !== 'intellij') {
    if (existing.npm?.script && merged.npm) merged.npm = { ...merged.npm, script: existing.npm.script }
    if (existing.spring) merged.spring = existing.spring
  }
  return merged
}

function findOrCreateGroup(ws: Workspace, name: string, newId: (p: string) => string): Group | undefined {
  const trimmed = name.trim()
  if (!trimmed) return undefined
  let g = ws.groups.find((x) => x.name.toLowerCase() === trimmed.toLowerCase())
  if (!g) {
    g = { id: newId('grp'), name: trimmed }
    ws.groups.push(g)
  }
  return g
}

export interface ImportOutcome {
  added: string[]
  updated: string[]
}

/** 선택한 후보를 워크스페이스에 넣는다 (ws를 직접 고친다). */
export function applyImport(
  ws: Workspace,
  scan: ScanResult,
  items: ImportItem[],
  varName: string | null,
  newId: (prefix: string) => string,
): ImportOutcome {
  const out: ImportOutcome = { added: [], updated: [] }
  if (varName) {
    ws.variables = { ...(ws.variables ?? {}), [varName]: scan.root }
  }
  const rootVar = varName ?? findRootVar(ws, scan.root)
  const taken = new Set(ws.services.map((s) => s.name))

  for (const item of items.filter((i) => i.selected)) {
    let svc = structuredClone(item.candidate.service)
    if (item.script && svc.npm) svc.npm.script = item.script
    if (item.profiles !== undefined && ['gradle-boot', 'maven-boot', 'jar'].includes(svc.type)) {
      const profiles = item.profiles.split(',').map((p) => p.trim()).filter(Boolean)
      if (profiles.length) svc.spring = { ...(svc.spring ?? {}), profiles }
      else delete svc.spring
    }
    if (rootVar) svc = applyRootVar(svc, scan.root, rootVar)

    const existingIdx = ws.services.findIndex((s) => s.source?.key === item.candidate.key)
    if (existingIdx >= 0) {
      ws.services[existingIdx] = mergeImported(ws.services[existingIdx], svc)
      out.updated.push(ws.services[existingIdx].id)
      continue
    }
    const group = findOrCreateGroup(ws, item.group, newId)
    svc.id = newId('svc')
    svc.name = uniqueName(item.name.trim() || svc.name, taken)
    taken.add(svc.name)
    if (group) svc.group = group.id
    else delete svc.group
    ws.services.push(svc)
    out.added.push(svc.id)
  }
  return out
}

/** 후보 목록으로 화면용 항목을 만든다. 이미 가져온 후보는 기본 선택하지 않는다. */
export function initialItems(ws: Workspace, scan: ScanResult): ImportItem[] {
  const existing = new Set(ws.services.map((s) => s.source?.key).filter(Boolean))
  return scan.candidates.map((c) => ({
    candidate: c,
    selected: c.recommended && !existing.has(c.key),
    name: c.service.name,
    group: c.group ?? '',
    script: c.service.npm?.script,
    profiles: c.service.spring?.profiles?.join(', ') ?? (c.profiles.length ? '' : undefined),
  }))
}
