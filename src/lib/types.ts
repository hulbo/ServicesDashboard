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
export type ClasspathSource = 'gradle' | 'maven' | 'manual'
export type RebuildPolicy = 'auto' | 'always' | 'never'

export interface Preset {
  id: string
  name: string
  services: string[]
  [extra: string]: unknown
}
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
  mainClass?: string
  runtime?: { jdk?: string; node?: string; maven?: string; [extra: string]: unknown }
  inject?: { utf8?: boolean; color?: boolean; [extra: string]: unknown }
  readiness?: { logPattern?: string; httpUrl?: string; timeoutSecs?: number; disabled?: boolean; [extra: string]: unknown }
  dependsOn?: string[]
  autoRestart?: { maxRetries?: number; delaySecs?: number; [extra: string]: unknown }
  debug?: { port?: number; suspend?: boolean; [extra: string]: unknown }
  javaMain?: { source?: ClasspathSource; module?: string; classpath?: string[]; rebuild?: RebuildPolicy; [extra: string]: unknown }
  spring?: { profiles?: string[]; [extra: string]: unknown }
  npm?: { script: string; packageManager?: PackageManager; [extra: string]: unknown }
  node?: { entry: string; nodeArgs?: string[]; [extra: string]: unknown }
  gradle?: { project?: string; tasks?: string[]; options?: string[]; daemon?: boolean; [extra: string]: unknown }
  maven?: { module?: string; goals?: string[]; options?: string[]; [extra: string]: unknown }
  jar?: { path: string; [extra: string]: unknown }
  shell?: { command: string; shell?: ShellKind; [extra: string]: unknown }
  ports?: number[]
  encoding?: OutputEncoding
  notes?: string
  source?: ServiceSource
  [extra: string]: unknown
}

/** 가져온 서비스의 원본 정보 (다시 가져오기에 쓴다) */
export interface ServiceSource {
  kind: SourceKind
  root: string
  key: string
  origin: string
}

export type SourceKind = 'intellij' | 'npm' | 'gradle' | 'maven' | 'vscode' | 'shared'

export interface PresetCandidate {
  key: string
  name: string
  origin: string
  members: string[]
  missing: string[]
}

export interface Candidate {
  key: string
  source: SourceKind
  origin: string
  group: string | null
  service: Service
  scripts: string[]
  profiles: string[]
  warnings: string[]
  recommended: boolean
  command: string | null
  commandError: string | null
}

export interface ScanResult {
  root: string
  suggestedVar: string
  candidates: Candidate[]
  presets: PresetCandidate[]
  warnings: string[]
}

export type RuntimeKind = 'jdk' | 'node' | 'maven'

export interface RuntimeInfo {
  kind: RuntimeKind
  path: string
  version: string
  major: string
  vendor: string | null
  source: string
}

export const SOURCE_LABELS: Record<SourceKind, string> = {
  shared: '프로젝트 공유 파일',
  intellij: 'IntelliJ 실행 구성',
  vscode: 'VS Code launch.json',
  npm: 'npm 스크립트',
  gradle: 'Spring Boot (Gradle)',
  maven: 'Spring Boot (Maven)',
}

export interface Workspace {
  $schema?: string
  name: string
  variables?: EnvMap
  env?: EnvMap
  groups: Group[]
  services: Service[]
  presets?: Preset[]
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

export type RunState = 'stopped' | 'waiting' | 'starting' | 'running' | 'stopping' | 'exited' | 'failed'

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
  ports: number[]
  urls: string[]
  readyAt: number | null
  debugPort: number | null
}

export interface PortOwner {
  port: number
  pid: number
  name: string
  serviceId: string | null
}

export interface AppSettings {
  currentWorkspace: string
  closeToTray: boolean
  notifications: boolean
  logFiles: { enabled: boolean; retentionDays: number }
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
  'java-main': 'Java 메인 클래스 (빠른 기동)',
  jar: '실행 jar',
  gradle: 'Gradle 태스크',
  maven: 'Maven 골',
  shell: '셸 명령',
}

export const JAVA_KINDS: ServiceKind[] = ['gradle-boot', 'maven-boot', 'java-main', 'jar', 'gradle', 'maven']
export const SPRING_KINDS: ServiceKind[] = ['gradle-boot', 'maven-boot', 'jar']
export const NODE_KINDS: ServiceKind[] = ['npm', 'node']
export const MAVEN_KINDS: ServiceKind[] = ['maven-boot', 'maven']
export const MAIN_CLASS_KINDS: ServiceKind[] = ['gradle-boot', 'maven-boot', 'java-main']
/** 디버그로 시작할 수 있는 타입 */
export const DEBUG_KINDS: ServiceKind[] = ['gradle-boot', 'maven-boot', 'java-main', 'jar', 'gradle', 'maven', 'npm', 'node']

/** 타입별 기본 준비 감지 (백엔드 watch.rs와 같은 내용을 사람이 읽기 쉽게) */
export const DEFAULT_READY_HINT: Partial<Record<ServiceKind, string>> = {
  'gradle-boot': 'Started … in N seconds (Spring Boot)',
  'maven-boot': 'Started … in N seconds (Spring Boot)',
  'java-main': 'Started … in N seconds (Spring Boot)',
  jar: 'Started … in N seconds (Spring Boot)',
  npm: 'Local: http://…, ready in, compiled successfully, listening on … 등',
  node: 'Local: http://…, ready in, compiled successfully, listening on … 등',
}

export const STATE_LABELS: Record<RunState, string> = {
  stopped: '중지됨',
  waiting: '대기 중',
  starting: '시작 중',
  running: '실행 중',
  stopping: '중지 중',
  exited: '종료됨',
  failed: '실패',
}

export function isActive(state: RunState | undefined): boolean {
  return state === 'running' || state === 'starting' || state === 'stopping' || state === 'waiting'
}
