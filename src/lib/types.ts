// 백엔드(Rust) 모델과 1:1로 맞춘 타입. src-tauri/src/model.rs 참고.

export type EnvMap = Record<string, string>

export type ServiceKind =
  | 'npm'
  | 'node'
  | 'gradle-boot'
  | 'maven-boot'
  | 'java-main'
  | 'jar'
  | 'gradle'
  | 'maven'
  | 'shell'

export type OutputEncoding = 'auto' | 'utf-8' | 'cp949'
export type PackageManager = 'npm' | 'yarn' | 'pnpm'
export type ShellKind = 'cmd' | 'powershell' | 'pwsh' | 'bash'

export interface Group {
  id: string
  name: string
  env?: EnvMap
  [extra: string]: unknown
}

export interface Service {
  id: string
  name: string
  group?: string
  type: ServiceKind
  cwd?: string
  env?: EnvMap
  envFiles?: string[]
  jvmArgs?: string[]
  args?: string[]
  runtime?: { jdk?: string; node?: string; [extra: string]: unknown }
  spring?: { profiles?: string[]; [extra: string]: unknown }
  npm?: { script: string; packageManager?: PackageManager; [extra: string]: unknown }
  node?: { entry: string; nodeArgs?: string[]; [extra: string]: unknown }
  gradle?: { project?: string; tasks?: string[]; options?: string[]; [extra: string]: unknown }
  maven?: { module?: string; goals?: string[]; options?: string[]; [extra: string]: unknown }
  jar?: { path: string; [extra: string]: unknown }
  shell?: { command: string; shell?: ShellKind; [extra: string]: unknown }
  ports?: number[]
  encoding?: OutputEncoding
  notes?: string
  [extra: string]: unknown
}

export interface Workspace {
  $schema?: string
  name: string
  variables?: EnvMap
  env?: EnvMap
  groups: Group[]
  services: Service[]
  [extra: string]: unknown
}

export interface WorkspaceInfo {
  workspace: Workspace
  path: string
}

export interface AppInfo {
  version: string
  configDir: string
  workspacePath: string
}

export type RunState = 'stopped' | 'starting' | 'running' | 'stopping' | 'exited' | 'failed'

export interface ServiceStatus {
  id: string
  state: RunState
  pid: number | null
  exitCode: number | null
  startedAt: number | null
  endedAt: number | null
  runId: number
  command: string | null
  error: string | null
}

export const STREAM_OUT = 0
export const STREAM_ERR = 1
export const STREAM_SYS = 2

export interface LogLine {
  seq: number
  ts: number
  stream: number
  text: string
}

export interface LogBatch {
  id: string
  lines: LogLine[]
}

export interface LaunchPreview {
  command: string
  cwd: string
  env: EnvMap
}

export interface StartError {
  id: string
  name: string
  error: string
}

export const KIND_LABELS: Record<ServiceKind, string> = {
  npm: 'npm 스크립트',
  node: 'Node 스크립트',
  'gradle-boot': 'Spring Boot (Gradle)',
  'maven-boot': 'Spring Boot (Maven)',
  'java-main': 'Java 메인 클래스 (M4 예정)',
  jar: '실행 jar',
  gradle: 'Gradle 태스크',
  maven: 'Maven 골',
  shell: '셸 명령',
}

export const JAVA_KINDS: ServiceKind[] = ['gradle-boot', 'maven-boot', 'java-main', 'jar', 'gradle', 'maven']
export const SPRING_KINDS: ServiceKind[] = ['gradle-boot', 'maven-boot', 'jar']
export const NODE_KINDS: ServiceKind[] = ['npm', 'node']

export const STATE_LABELS: Record<RunState, string> = {
  stopped: '중지됨',
  starting: '시작 중',
  running: '실행 중',
  stopping: '중지 중',
  exited: '종료됨',
  failed: '실패',
}

export function isActive(state: RunState | undefined): boolean {
  return state === 'running' || state === 'starting' || state === 'stopping'
}
