// 앱 전역 상태 (Svelte 5 runes)

import { createBackend, errorMessage, type Backend } from './api'
import { detectHeadChanges, headLabel } from './git'
import { applyImport, type ImportItem, type PresetItem } from './importing'
import { LogStore } from './logstore'
import type { AppInfo, AppSettings, GitRepo, Group, LogBatch, PortOwner, Preset, RuntimeInfo, ScanResult, Service, ServiceStatus, StartError, Workspace } from './types'
import { isActive } from './types'

export type Selection = { kind: 'service' | 'group' | 'preset'; id: string } | { kind: 'workspace' | 'import' | 'runtimes' | 'settings' | 'branches' }

/** 시작하려는데 포트가 점유된 경우 보여 줄 대화상자 내용 */
export interface PortConflict {
  id: string
  owners: PortOwner[]
}
export type DetailTab = 'console' | 'settings'

export interface Toast {
  id: number
  kind: 'info' | 'success' | 'error'
  text: string
  /** 알림 안의 버튼 (누르면 알림도 닫힌다) */
  action?: { label: string; run: () => void }
}

/** 밖에서 바꾼 브랜치를 확인하는 주기 */
const GIT_POLL_MS = 3000

const COLLAPSE_KEY = 'sd.collapsed'
const SIDEBAR_KEY = 'sd.sidebarWidth'

function readLocal<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    return raw ? (JSON.parse(raw) as T) : fallback
  } catch {
    return fallback
  }
}

function writeLocal(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // 저장 공간을 쓸 수 없으면 무시 (화면 편의 기능일 뿐이다)
  }
}

export function newId(prefix: string): string {
  const rand = typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID().slice(0, 8) : Math.random().toString(36).slice(2, 10)
  return `${prefix}-${rand}`
}

class AppState {
  backend!: Backend
  ready = $state(false)
  loadError = $state<string | null>(null)
  info = $state<AppInfo | null>(null)
  workspace = $state<Workspace>({ name: '', groups: [], services: [] })
  workspacePath = $state('')
  statuses = $state<Record<string, ServiceStatus>>({})
  selection = $state<Selection | null>(null)
  tab = $state<DetailTab>('console')
  collapsed = $state<Record<string, boolean>>(readLocal(COLLAPSE_KEY, {}))
  sidebarWidth = $state<number>(readLocal(SIDEBAR_KEY, 300))
  renaming = $state<string | null>(null)
  toasts = $state<Toast[]>([])
  /** 서비스별 로그 변경 카운터. ConsoleView가 이 값을 보고 다시 그린다. */
  logTick = $state<Record<string, number>>({})
  /** 1초마다 갱신되는 현재 시각 (실행 시간 표시용) */
  now = $state(Date.now())
  runtimes = $state<RuntimeInfo[]>([])
  runtimesLoading = $state(false)
  /** 가져오기 화면에서 마지막으로 스캔한 폴더 */
  importRoot = $state('')
  settings = $state<AppSettings | null>(null)
  conflict = $state<PortConflict | null>(null)
  /** 서비스가 쓰는 git 저장소와 현재 브랜치 */
  repos = $state<GitRepo[]>([])
  /** 브랜치 화면에서 펼친 저장소 */
  branchFocus = $state<string | null>(null)
  /** 이 앱이 브랜치를 바꾸는 중인 저장소 */
  switching = $state<Record<string, boolean>>({})

  /** 저장소별 마지막으로 본 HEAD (밖에서 바꾼 브랜치 감지용) */
  private heads = new Map<string, string>()
  private logStores = new Map<string, LogStore>()
  private logsLoaded = new Set<string>()
  private toastSeq = 1

  selectedService = $derived.by(() => {
    const sel = this.selection
    return sel?.kind === 'service' ? this.workspace.services.find((s) => s.id === sel.id) : undefined
  })

  selectedGroup = $derived.by(() => {
    const sel = this.selection
    return sel?.kind === 'group' ? this.workspace.groups.find((g) => g.id === sel.id) : undefined
  })

  selectedPreset = $derived.by(() => {
    const sel = this.selection
    return sel?.kind === 'preset' ? (this.workspace.presets ?? []).find((p) => p.id === sel.id) : undefined
  })

  runningCount = $derived(Object.values(this.statuses).filter((s) => isActive(s.state) && this.workspace.services.some((x) => x.id === s.id)).length)

  async init() {
    this.backend = await createBackend()
    await this.backend.onState((s) => this.applyStatus(s))
    await this.backend.onLogs((batches) => this.applyLogs(batches))
    await this.backend.onStartError((e) => void this.handleStartError(e))
    setInterval(() => {
      if (this.runningCount > 0) this.now = Date.now()
    }, 1000)
    try {
      this.info = await this.backend.appInfo()
    } catch (e) {
      this.toast('error', errorMessage(e))
    }
    await this.loadWorkspace(false)
    for (const s of await this.backend.statuses()) this.applyStatus(s)
    this.ready = true
    void this.refreshRepos()
    setInterval(() => {
      if (!document.hidden) void this.refreshRepos()
    }, GIT_POLL_MS)
    // 트레이에 숨어 있던 동안 바뀐 브랜치도 창을 다시 열면 바로 알린다
    document.addEventListener('visibilitychange', () => {
      if (!document.hidden) void this.refreshRepos()
    })
    void this.loadRuntimes(false)
    try {
      this.settings = await this.backend.settingsGet()
    } catch (e) {
      this.toast('error', `설정을 읽지 못했습니다: ${errorMessage(e)}`)
    }
  }

  async saveSettings(next: AppSettings) {
    try {
      this.settings = await this.backend.settingsSave(next)
      return true
    } catch (e) {
      this.toast('error', `설정 저장 실패: ${errorMessage(e)}`)
      return false
    }
  }

  async loadRuntimes(refresh: boolean) {
    this.runtimesLoading = true
    try {
      this.runtimes = await this.backend.detectRuntimes(refresh)
    } catch (e) {
      this.toast('error', `런타임 감지 실패: ${errorMessage(e)}`)
    } finally {
      this.runtimesLoading = false
    }
  }

  async loadWorkspace(reload: boolean) {
    try {
      const info = reload ? await this.backend.reloadWorkspace() : await this.backend.getWorkspace()
      this.workspace = info.workspace
      this.workspacePath = info.path
      this.loadError = null
      if (this.selection && 'id' in this.selection) {
        const id = this.selection.id
        const exists =
          this.selection.kind === 'service'
            ? info.workspace.services.some((s) => s.id === id)
            : this.selection.kind === 'preset'
              ? (info.workspace.presets ?? []).some((p) => p.id === id)
              : info.workspace.groups.some((g) => g.id === id)
        if (!exists) this.selection = null
      }
      if (!this.selection && info.workspace.services.length) {
        this.selection = { kind: 'service', id: info.workspace.services[0].id }
      }
    } catch (e) {
      this.loadError = errorMessage(e)
    }
  }

  // ---------- 상태·로그 ----------

  private applyStatus(s: ServiceStatus) {
    this.statuses[s.id] = s
    this.now = Date.now()
  }

  private applyLogs(batches: LogBatch[]) {
    for (const b of batches) {
      const store = this.logStore(b.id)
      if (b.reset) store.clear()
      const added = store.append(b.lines)
      if (added || b.reset) this.logTick[b.id] = (this.logTick[b.id] ?? 0) + 1
    }
  }

  logStore(id: string): LogStore {
    let store = this.logStores.get(id)
    if (!store) {
      store = new LogStore()
      this.logStores.set(id, store)
    }
    return store
  }

  /** 화면을 새로 고친 경우 등을 대비해 처음 볼 때 백엔드 버퍼를 한 번 받아 온다. */
  async ensureLogs(id: string) {
    if (this.logsLoaded.has(id)) return
    this.logsLoaded.add(id)
    try {
      const lines = await this.backend.getLogs(id)
      this.logStore(id).reset(lines)
      this.logTick[id] = (this.logTick[id] ?? 0) + 1
    } catch (e) {
      this.logsLoaded.delete(id)
      this.toast('error', errorMessage(e))
    }
  }

  async clearLogs(id: string) {
    await this.backend.clearLogs(id)
    this.logStore(id).clear()
    this.logTick[id] = (this.logTick[id] ?? 0) + 1
  }

  status(id: string): ServiceStatus | undefined {
    return this.statuses[id]
  }

  // ---------- 실행 제어 ----------

  private async run(label: string, fn: () => Promise<unknown>) {
    try {
      await fn()
    } catch (e) {
      this.toast('error', `${label}: ${errorMessage(e)}`)
    }
  }

  serviceName(id: string): string {
    return this.workspace.services.find((s) => s.id === id)?.name ?? id
  }

  /** 포트 충돌이면 대화상자를 띄우고, 아니면 일반 오류로 알린다. */
  private async startOrConflict(id: string, fn: () => Promise<unknown>, label: string) {
    try {
      await fn()
    } catch (e) {
      const msg = errorMessage(e)
      if (msg.startsWith('포트가 이미 사용 중')) {
        const owners = await this.backend.portsCheck(id).catch(() => [])
        if (owners.length) {
          this.conflict = { id, owners }
          return
        }
      }
      this.toast('error', `${this.serviceName(id)} ${label} 실패: ${msg}`)
    }
  }

  start(id: string, debug = false) {
    return this.startOrConflict(id, () => this.backend.start(id, debug), debug ? '디버그 시작' : '시작')
  }

  /** 백그라운드 시작(의존 순서·그룹·프리셋·자동 재시작)에서 온 실패 */
  private async handleStartError(e: StartError) {
    if (e.error.startsWith('포트가 이미 사용 중') && !this.conflict) {
      const owners = await this.backend.portsCheck(e.id).catch(() => [] as PortOwner[])
      if (owners.length) {
        this.conflict = { id: e.id, owners }
        return
      }
    }
    this.toast('error', `${e.name} 시작 실패: ${e.error}`)
  }

  /** 충돌 대화상자에서 '종료하고 시작'을 눌렀을 때 */
  async resolveConflict(kill: boolean) {
    const c = this.conflict
    this.conflict = null
    if (!c || !kill) return
    try {
      for (const pid of new Set(c.owners.map((o) => o.pid))) await this.backend.processKill(pid)
      // 포트가 실제로 풀릴 때까지 잠시 기다린다
      for (let i = 0; i < 20; i++) {
        if ((await this.backend.portsCheck(c.id)).length === 0) break
        await new Promise((r) => setTimeout(r, 250))
      }
    } catch (e) {
      this.toast('error', `프로세스 종료 실패: ${errorMessage(e)}`)
      return
    }
    await this.start(c.id)
  }

  stop(id: string) {
    return this.run(`${this.serviceName(id)} 중지 실패`, () => this.backend.stop(id))
  }

  restart(id: string, debug = false) {
    return this.startOrConflict(id, () => this.backend.restart(id, debug), '재시작')
  }

  toggle(id: string) {
    return isActive(this.statuses[id]?.state) ? this.stop(id) : this.start(id)
  }

  startGroup(id: string) {
    return this.run('그룹 시작 실패', () => this.backend.startGroup(id))
  }

  startPreset(id: string) {
    return this.run('프리셋 시작 실패', () => this.backend.startPreset(id))
  }

  stopPreset(id: string) {
    return this.run('프리셋 중지 실패', () => this.backend.stopPreset(id))
  }

  async addPreset(services: string[] = []) {
    const id = newId('pre')
    const ok = await this.mutate((ws) => {
      ws.presets = [...(ws.presets ?? []), { id, name: '새 프리셋', services }]
    })
    if (ok) {
      this.select({ kind: 'preset', id })
      this.renaming = id
    }
  }

  async updatePreset(preset: Preset) {
    return this.mutate((ws) => {
      ws.presets = (ws.presets ?? []).map((p) => (p.id === preset.id ? preset : p))
    })
  }

  async deletePreset(id: string) {
    const p = (this.workspace.presets ?? []).find((x) => x.id === id)
    if (!p || !(await this.backend.confirm(`'${p.name}' 프리셋을 삭제할까요? (서비스는 지워지지 않습니다)`, '프리셋 삭제'))) return
    const ok = await this.mutate((ws) => {
      ws.presets = (ws.presets ?? []).filter((x) => x.id !== id)
    })
    if (ok && this.selection?.kind === 'preset' && this.selection.id === id) this.selection = null
  }

  async exportProject(root: string) {
    try {
      const path = await this.backend.projectExport(root)
      this.toast('success', `공유 파일을 만들었습니다: ${path}`)
      return true
    } catch (e) {
      this.toast('error', `내보내기 실패: ${errorMessage(e)}`)
      return false
    }
  }

  stopGroup(id: string) {
    return this.run('그룹 중지 실패', () => this.backend.stopGroup(id))
  }

  stopAll() {
    return this.run('전체 중지 실패', () => this.backend.stopAll())
  }

  // ---------- 워크스페이스 편집 ----------

  /** 사본을 고쳐 저장하고, 저장된 결과로 상태를 바꾼다. */
  async mutate(fn: (ws: Workspace) => void): Promise<boolean> {
    const draft = $state.snapshot(this.workspace) as Workspace
    fn(draft)
    try {
      this.workspace = await this.backend.saveWorkspace(draft)
      return true
    } catch (e) {
      this.toast('error', `저장 실패: ${errorMessage(e)}`)
      return false
    }
  }

  async addService(groupId?: string) {
    const id = newId('svc')
    const ok = await this.mutate((ws) => {
      ws.services.push({ id, name: '새 서비스', type: 'npm', group: groupId, npm: { script: 'dev' } })
    })
    if (ok) {
      this.select({ kind: 'service', id })
      this.tab = 'settings'
      if (groupId) this.collapsed[groupId] = false
    }
  }

  async addGroup() {
    const id = newId('grp')
    const ok = await this.mutate((ws) => {
      ws.groups.push({ id, name: '새 그룹' })
    })
    if (ok) {
      this.select({ kind: 'group', id })
      this.renaming = id
    }
  }

  async updateService(svc: Service) {
    return this.mutate((ws) => {
      const i = ws.services.findIndex((s) => s.id === svc.id)
      if (i >= 0) ws.services[i] = svc
    })
  }

  async updateGroup(group: Group) {
    return this.mutate((ws) => {
      const i = ws.groups.findIndex((g) => g.id === group.id)
      if (i >= 0) ws.groups[i] = group
    })
  }

  async rename(id: string, name: string) {
    const trimmed = name.trim()
    this.renaming = null
    if (!trimmed) return
    await this.mutate((ws) => {
      const s = ws.services.find((x) => x.id === id)
      if (s) s.name = trimmed
      const g = ws.groups.find((x) => x.id === id)
      if (g) g.name = trimmed
      const p = (ws.presets ?? []).find((x) => x.id === id)
      if (p) p.name = trimmed
    })
  }

  async duplicateService(id: string) {
    const src = this.workspace.services.find((s) => s.id === id)
    if (!src) return
    const copy = { ...($state.snapshot(src) as Service), id: newId('svc'), name: `${src.name} 복사본` } as Service
    delete (copy as Record<string, unknown>).source
    const ok = await this.mutate((ws) => {
      const i = ws.services.findIndex((s) => s.id === id)
      ws.services.splice(i + 1, 0, copy)
    })
    if (ok) this.select({ kind: 'service', id: copy.id })
  }

  async deleteService(id: string) {
    const name = this.serviceName(id)
    const running = isActive(this.statuses[id]?.state)
    const msg = running ? `'${name}'이(가) 실행 중입니다. 중지하고 삭제할까요?` : `'${name}' 서비스를 삭제할까요?`
    if (!(await this.backend.confirm(msg, '서비스 삭제'))) return
    if (running) await this.stop(id)
    const ok = await this.mutate((ws) => {
      ws.services = ws.services.filter((s) => s.id !== id)
      // 다른 서비스의 의존 대상·프리셋에서도 뺀다
      for (const s of ws.services) if (s.dependsOn) s.dependsOn = s.dependsOn.filter((d) => d !== id)
      for (const p of ws.presets ?? []) p.services = p.services.filter((x) => x !== id)
    })
    if (ok && this.selection?.kind === 'service' && this.selection.id === id) this.selection = null
  }

  async deleteGroup(id: string) {
    const group = this.workspace.groups.find((g) => g.id === id)
    if (!group) return
    const count = this.workspace.services.filter((s) => s.group === id).length
    const msg = count
      ? `'${group.name}' 그룹을 삭제할까요? 그룹 안의 서비스 ${count}개는 그룹 없음으로 옮겨집니다.`
      : `'${group.name}' 그룹을 삭제할까요?`
    if (!(await this.backend.confirm(msg, '그룹 삭제'))) return
    const ok = await this.mutate((ws) => {
      ws.groups = ws.groups.filter((g) => g.id !== id)
      for (const s of ws.services) if (s.group === id) delete s.group
    })
    if (ok && this.selection?.kind === 'group' && this.selection.id === id) this.selection = null
  }

  /** 서비스를 다른 서비스 앞(beforeId) 또는 그룹 끝으로 옮긴다. groupId가 null이면 그룹 없음. */
  async moveService(id: string, groupId: string | null, beforeId?: string) {
    if (id === beforeId) return
    await this.mutate((ws) => {
      const i = ws.services.findIndex((s) => s.id === id)
      if (i < 0) return
      const [svc] = ws.services.splice(i, 1)
      if (groupId) svc.group = groupId
      else delete svc.group
      let at = beforeId ? ws.services.findIndex((s) => s.id === beforeId) : -1
      if (at < 0) {
        // 그룹의 마지막 서비스 뒤에 넣는다
        const lastInGroup = ws.services.map((s) => s.group ?? null).lastIndexOf(groupId)
        at = lastInGroup >= 0 ? lastInGroup + 1 : ws.services.length
      }
      ws.services.splice(at, 0, svc)
    })
  }

  async moveGroup(id: string, beforeId?: string) {
    if (id === beforeId) return
    await this.mutate((ws) => {
      const i = ws.groups.findIndex((g) => g.id === id)
      if (i < 0) return
      const [g] = ws.groups.splice(i, 1)
      const at = beforeId ? ws.groups.findIndex((x) => x.id === beforeId) : -1
      ws.groups.splice(at < 0 ? ws.groups.length : at, 0, g)
    })
  }

  // ---------- 가져오기 ----------

  /** 스캔 결과에서 고른 후보를 워크스페이스에 넣는다. */
  async importItems(scan: ScanResult, items: ImportItem[], varName: string | null, presetItems: PresetItem[] = []) {
    let outcome = { added: [] as string[], updated: [] as string[], presets: [] as string[] }
    const ok = await this.mutate((ws) => {
      outcome = applyImport(ws, scan, items, varName, newId, presetItems)
    })
    if (!ok) return false
    const parts = []
    if (outcome.added.length) parts.push(`${outcome.added.length}개 추가`)
    if (outcome.updated.length) parts.push(`${outcome.updated.length}개 갱신`)
    if (outcome.presets.length) parts.push(`프리셋 ${outcome.presets.length}개`)
    this.toast('success', `서비스를 가져왔습니다 (${parts.join(', ') || '변경 없음'}).`)
    const first = outcome.added[0] ?? outcome.updated[0]
    if (first) {
      for (const g of this.workspace.groups) this.collapsed[g.id] = false
      this.select({ kind: 'service', id: first })
      this.tab = 'settings'
    }
    return true
  }

  /** 가져온 서비스를 원본(IntelliJ 구성 등)에서 다시 읽어 실행 정의를 갱신한다. */
  async resync(id: string) {
    const svc = this.workspace.services.find((s) => s.id === id)
    const src = svc?.source
    if (!svc || !src?.root || !src.key) {
      this.toast('error', '가져온 출처 정보가 없는 서비스입니다.')
      return
    }
    try {
      const scan = await this.backend.scanProject(src.root)
      const cand = scan.candidates.find((c) => c.key === src.key)
      if (!cand) {
        this.toast('error', `원본 구성을 찾지 못했습니다: ${src.origin} (${src.root})`)
        return
      }
      const item: ImportItem = { candidate: cand, selected: true, name: svc.name, group: '', targetId: id }
      let changed = false
      const ok = await this.mutate((ws) => {
        const before = JSON.stringify(ws.services.find((s) => s.id === id))
        applyImport(ws, scan, [item], null, newId)
        changed = before !== JSON.stringify(ws.services.find((s) => s.id === id))
      })
      if (ok) this.toast('success', changed ? `'${svc.name}'을(를) 원본에서 다시 가져왔습니다.` : '원본과 같아 바뀐 내용이 없습니다.')
    } catch (e) {
      this.toast('error', `다시 가져오기 실패: ${errorMessage(e)}`)
    }
  }

  // ---------- 화면 ----------

  select(sel: Selection | null) {
    this.selection = sel
    if (sel?.kind === 'service') void this.ensureLogs(sel.id)
  }

  toggleCollapsed(groupId: string) {
    this.collapsed[groupId] = !this.collapsed[groupId]
    writeLocal(COLLAPSE_KEY, $state.snapshot(this.collapsed))
  }

  setSidebarWidth(w: number) {
    this.sidebarWidth = Math.round(Math.min(600, Math.max(200, w)))
    writeLocal(SIDEBAR_KEY, this.sidebarWidth)
  }

  toast(kind: Toast['kind'], text: string, action?: Toast['action']) {
    const id = this.toastSeq++
    this.toasts.push({ id, kind, text, action })
    // 버튼이 있는 알림은 누를 시간을 넉넉히 준다
    setTimeout(() => this.dismiss(id), action ? 30000 : kind === 'error' ? 8000 : 4000)
  }

  // ---------- git 브랜치 ----------

  repoOf(serviceId: string): GitRepo | undefined {
    return this.repos.find((r) => r.services.includes(serviceId))
  }

  openBranches(root?: string) {
    this.branchFocus = root ?? (this.repos.length === 1 ? this.repos[0].root : this.branchFocus)
    this.select({ kind: 'branches' })
  }

  /** 저장소 목록을 새로 읽고, 밖(IntelliJ·터미널)에서 바뀐 브랜치가 있으면 알린다. */
  async refreshRepos() {
    let list: GitRepo[]
    try {
      list = await this.backend.gitRepos()
    } catch {
      return // 워크스페이스 파일 오류 등: 브랜치 표시만 건너뛴다
    }
    const changed = detectHeadChanges(this.heads, list, new Set(Object.keys(this.switching)))
    if (JSON.stringify(list) !== JSON.stringify($state.snapshot(this.repos))) this.repos = list
    for (const c of changed) this.onExternalSwitch(c.repo, c.from)
  }

  private onExternalSwitch(repo: GitRepo, from: string) {
    const running = repo.services.filter((id) => isActive(this.statuses[id]?.state))
    const text = `${repo.name} 브랜치가 바뀌었습니다: ${from} → ${headLabel(repo)}`
    if (!running.length) {
      this.toast('info', text)
      return
    }
    this.toast('info', `${text} · 실행 중인 서비스 ${running.length}개는 이전 코드로 떠 있습니다.`, {
      label: `${running.length}개 재시작`,
      run: () => void this.restartMany(running),
    })
  }

  async restartMany(ids: string[]) {
    for (const id of ids) await this.restart(id)
  }

  /** 브랜치 전환 (실행 중인 서비스 중지 → 전환 → pull → 다시 시작은 백엔드가 한다) */
  async switchBranch(repo: GitRepo, branch: string, pull: boolean, restart: boolean): Promise<boolean> {
    this.switching[repo.root] = true
    try {
      const r = await this.backend.gitSwitch(repo.root, branch, pull, restart)
      const n = r.stopped.length
      const tail = n ? (r.restarted ? ` · 서비스 ${n}개 다시 시작` : ` · 서비스 ${n}개 중지됨`) : ''
      this.toast('success', `${repo.name}: ${r.branch} 브랜치로 전환했습니다${pull ? ' (pull 포함)' : ''}${tail}`)
      for (const w of r.warnings) this.toast('error', w)
      return true
    } catch (e) {
      this.toast('error', `브랜치 전환 실패: ${errorMessage(e)}`)
      return false
    } finally {
      // 기록을 새 브랜치로 맞춘 뒤에 표시를 풀어야 '밖에서 바뀜' 알림이 뜨지 않는다
      await this.refreshRepos()
      delete this.switching[repo.root]
    }
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id)
  }
}

export const app = new AppState()
