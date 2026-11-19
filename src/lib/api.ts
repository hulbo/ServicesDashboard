// 백엔드 호출 계층.
// Tauri 앱 안에서는 Rust 명령을 호출하고, 일반 브라우저(개발 미리보기)에서는 가짜 백엔드를 쓴다.

import type {
  AppInfo,
  LaunchPreview,
  LogBatch,
  LogLine,
  AppSettings,
  PortOwner,
  RuntimeInfo,
  ScanResult,
  Service,
  ServiceStatus,
  StartError,
  Workspace,
  WorkspaceInfo,
} from './types'

export interface Backend {
  readonly isMock: boolean
  appInfo(): Promise<AppInfo>
  getWorkspace(): Promise<WorkspaceInfo>
  reloadWorkspace(): Promise<WorkspaceInfo>
  saveWorkspace(ws: Workspace): Promise<Workspace>
  statuses(): Promise<ServiceStatus[]>
  start(id: string): Promise<ServiceStatus>
  stop(id: string): Promise<void>
  restart(id: string): Promise<ServiceStatus>
  startGroup(id: string): Promise<StartError[]>
  stopGroup(id: string): Promise<void>
  stopAll(): Promise<void>
  getLogs(id: string): Promise<LogLine[]>
  clearLogs(id: string): Promise<void>
  preview(service: Service): Promise<LaunchPreview>
  scanProject(root: string): Promise<ScanResult>
  detectRuntimes(refresh: boolean): Promise<RuntimeInfo[]>
  portsCheck(id: string): Promise<PortOwner[]>
  processKill(pid: number): Promise<void>
  settingsGet(): Promise<AppSettings>
  settingsSave(settings: AppSettings): Promise<AppSettings>
  logsExport(id: string, path: string): Promise<number>
  logsDir(id: string): Promise<string>
  pickSaveFile(defaultName: string): Promise<string | null>
  openPath(path: string): Promise<void>
  openUrl(url: string): Promise<void>
  pickFolder(defaultPath?: string): Promise<string | null>
  pickFile(defaultPath?: string, extensions?: string[]): Promise<string | null>
  confirm(message: string, title?: string): Promise<boolean>
  onState(cb: (s: ServiceStatus) => void): Promise<() => void>
  onLogs(cb: (b: LogBatch[]) => void): Promise<() => void>
}

export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  return JSON.stringify(e)
}

async function createTauriBackend(): Promise<Backend> {
  const { invoke } = await import('@tauri-apps/api/core')
  const { listen } = await import('@tauri-apps/api/event')
  const dialog = await import('@tauri-apps/plugin-dialog')

  const call = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    try {
      return await invoke<T>(cmd, args)
    } catch (e) {
      throw new Error(errorMessage(e))
    }
  }

  return {
    isMock: false,
    appInfo: () => call('app_info'),
    getWorkspace: () => call('workspace_get'),
    reloadWorkspace: () => call('workspace_reload'),
    saveWorkspace: (workspace) => call('workspace_save', { workspace }),
    statuses: () => call('service_statuses'),
    start: (id) => call('service_start', { id }),
    stop: (id) => call('service_stop', { id }),
    restart: (id) => call('service_restart', { id }),
    startGroup: (id) => call('group_start', { id }),
    stopGroup: (id) => call('group_stop', { id }),
    stopAll: () => call('stop_all'),
    getLogs: (id) => call('logs_get', { id }),
    clearLogs: (id) => call('logs_clear', { id }),
    preview: (service) => call('launch_preview', { service }),
    scanProject: (root) => call('project_scan', { root }),
    detectRuntimes: (refresh) => call('runtimes_detect', { refresh }),
    portsCheck: (id) => call('ports_check', { id }),
    processKill: (pid) => call('process_kill', { pid }),
    settingsGet: () => call('settings_get'),
    settingsSave: (settings) => call('settings_save', { settings }),
    logsExport: (id, path) => call('logs_export', { id, path }),
    logsDir: (id) => call('logs_dir', { id }),
    async pickSaveFile(defaultName) {
      const r = await dialog.save({ defaultPath: defaultName, filters: [{ name: '로그', extensions: ['log', 'txt'] }] })
      return typeof r === 'string' ? r : null
    },
    openPath: (path) => call('open_path', { path }),
    openUrl: (url) => call('open_url', { url }),
    async pickFolder(defaultPath) {
      const r = await dialog.open({ directory: true, multiple: false, defaultPath: defaultPath || undefined })
      return typeof r === 'string' ? r : null
    },
    async pickFile(defaultPath, extensions) {
      const r = await dialog.open({
        directory: false,
        multiple: false,
        defaultPath: defaultPath || undefined,
        filters: extensions?.length ? [{ name: extensions.join(', '), extensions }] : undefined,
      })
      return typeof r === 'string' ? r : null
    },
    confirm: (message, title) =>
      dialog.ask(message, { title: title ?? 'Services Dashboard', kind: 'warning', okLabel: '확인', cancelLabel: '취소' }),
    onState: (cb) => listen<ServiceStatus>('service://state', (e) => cb(e.payload)),
    onLogs: (cb) => listen<LogBatch[]>('service://logs', (e) => cb(e.payload)),
  }
}

export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export async function createBackend(): Promise<Backend> {
  if (inTauri()) return createTauriBackend()
  const { createMockBackend } = await import('./mock')
  return createMockBackend()
}
