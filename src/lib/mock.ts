// 브라우저 미리보기용 가짜 백엔드.
// `npm run dev`로 띄운 화면을 일반 브라우저에서 열면 이 백엔드가 쓰인다 (실제 프로세스는 실행하지 않음).

import type { Backend } from './api'
import { joinArgs } from './args'
import type { AppSettings, Candidate, LogBatch, LogLine, RuntimeInfo, ScanResult, Service, ServiceStatus, StartError, UpdateProgress, Workspace } from './types'
import { STREAM_ERR, STREAM_OUT, STREAM_SYS, isActive } from './types'

const sample: Workspace = {
  name: 'sample',
  variables: { ROOT: 'C:/Project/sample' },
  env: {},
  groups: [
    { id: 'npm', name: 'npm' },
    { id: 'server', name: 'server', env: { SPRING_PROFILES_ACTIVE: 'local' } },
  ],
  services: [
    { id: 'portal-front', name: 'portal', group: 'npm', type: 'npm', cwd: '${var:ROOT}/front/portal', npm: { script: 'dev' }, ports: [5173] },
    { id: 'sso-front', name: 'sso', group: 'npm', type: 'npm', cwd: '${var:ROOT}/front/sso', npm: { script: 'dev' }, ports: [5174] },
    { id: 'admin-front', name: 'admin', group: 'npm', type: 'npm', cwd: '${var:ROOT}/front/admin', npm: { script: 'serve', packageManager: 'pnpm' } },
    {
      id: 'portal-api',
      name: 'PortalApplication (local)',
      group: 'server',
      type: 'gradle-boot',
      cwd: '${var:ROOT}',
      gradle: { project: ':apps:portal' },
      spring: { profiles: ['local'] },
      jvmArgs: ['--add-opens', 'java.base/java.lang=ALL-UNNAMED', '-Dfile.encoding=UTF-8'],
      runtime: { jdk: 'C:/Program Files/Java/jdk-17' },
      ports: [8080],
      dependsOn: ['auth-api'],
    },
    { id: 'auth-api', name: 'AuthApplication', group: 'server', type: 'maven-boot', cwd: '${var:ROOT}/auth', maven: { module: 'auth-api' }, ports: [8081], autoRestart: { maxRetries: 3 } },
    { id: 'batch', name: 'batch-worker', type: 'shell', shell: { command: 'docker compose up redis', shell: 'cmd' } },
  ],
  presets: [{ id: 'pre-full', name: '풀스택', services: ['portal-front', 'portal-api'] }],
}

const viteLines = [
  '',
  '> portal@0.0.0 dev',
  '> vite',
  '',
  '',
  '  \x1b[32m\x1b[1mVITE\x1b[22m v5.4.10\x1b[39m  \x1b[2mready in \x1b[0m\x1b[1m812\x1b[22m\x1b[2m\x1b[0m ms\x1b[22m',
  '',
  '  \x1b[32m➜\x1b[39m  \x1b[1mLocal\x1b[22m:   \x1b[36mhttp://localhost:\x1b[1m5173\x1b[22m/\x1b[39m',
  '  \x1b[32m➜\x1b[39m  \x1b[1mNetwork\x1b[22m: \x1b[2muse \x1b[22m\x1b[1m--host\x1b[22m\x1b[2m to expose\x1b[22m',
]

const bootLines = [
  '> Task :apps:portal:compileJava UP-TO-DATE',
  '> Task :apps:portal:processResources UP-TO-DATE',
  '> Task :apps:portal:classes UP-TO-DATE',
  '> Task :apps:portal:bootRun',
  '',
  '  .   ____          _            __ _ _',
  ' /\\\\ / ___\'_ __ _ _(_)_ __  __ _ \\ \\ \\ \\',
  "( ( )\\___ | '_ | '_| | '_ \\/ _` | \\ \\ \\ \\",
  " \\\\/  ___)| |_)| | | | | || (_| |  ) ) ) )",
  "  '  |____| .__|_| |_|_| |_\\__, | / / / /",
  ' =========|_|==============|___/=/_/_/_/',
  '',
  ' :: Spring Boot ::                (v3.3.4)',
  '',
]

function bootLog(level: 'INFO' | 'WARN' | 'ERROR' | 'DEBUG', logger: string, msg: string): string {
  const color = { INFO: 32, WARN: 33, ERROR: 31, DEBUG: 36 }[level]
  const ts = new Date().toISOString().replace('Z', '+09:00')
  return `\x1b[2m${ts}\x1b[0;39m \x1b[${color}m${level.padStart(5)}\x1b[0;39m \x1b[35m12345\x1b[0;39m \x1b[2m---\x1b[0;39m \x1b[2m[portal] [  restartedMain]\x1b[0;39m \x1b[36m${logger.padEnd(40)}\x1b[0;39m \x1b[2m:\x1b[0;39m ${msg}`
}

const mockRuntimes: RuntimeInfo[] = [
  { kind: 'jdk', path: 'C:/Program Files/Java/jdk-17.0.15', version: '17.0.15', major: '17', vendor: 'Oracle Corporation', source: 'JAVA_HOME' },
  { kind: 'jdk', path: 'C:/Program Files/Eclipse Adoptium/jdk-21.0.2.13-hotspot', version: '21.0.2', major: '21', vendor: 'Eclipse Adoptium', source: '설치됨' },
  { kind: 'jdk', path: 'C:/Program Files/Java/jdk1.8.0_202', version: '1.8.0_202', major: '8', vendor: null, source: '설치됨' },
  { kind: 'node', path: 'C:/Program Files/nodejs', version: '22.11.0', major: '22', vendor: null, source: 'PATH' },
  { kind: 'node', path: 'C:/nvm/v20.11.1', version: '20.11.1', major: '20', vendor: null, source: 'nvm' },
  { kind: 'maven', path: 'C:/tools/apache-maven-3.9.6', version: '3.9.6', major: '3', vendor: null, source: 'MAVEN_HOME' },
]

function mockScan(root: string): ScanResult {
  const r = root.replace(/\\/g, '/').replace(/\/+$/, '')
  const mk = (
    key: string,
    source: Candidate['source'],
    origin: string,
    group: string | null,
    service: Partial<Service>,
    extra: Partial<Candidate> = {},
  ): Candidate => ({
    key,
    source,
    origin,
    group,
    service: { id: key, name: 'svc', type: 'npm', source: { kind: source, root: r, key, origin }, ...service } as Service,
    scripts: [],
    profiles: [],
    warnings: [],
    recommended: source === 'intellij',
    command: null,
    commandError: null,
    ...extra,
  })
  return {
    root: r,
    suggestedVar: (r.split('/').pop() || 'ROOT').toUpperCase().replace(/[^A-Z0-9]+/g, '_'),
    warnings: ['가져오지 않은 VS Code 구성: attach (attach)'],
    presets: [{ key: 'intellij-compound:.run/all.run.xml#all', name: 'all', origin: '.run/all.run.xml', members: ['intellij:.idea/workspace.xml#PortalApplication', 'intellij:.run/npm-portal.run.xml#portal'], missing: [] }],
    candidates: [
      mk(
        'intellij:.idea/workspace.xml#PortalApplication',
        'intellij',
        '.idea/workspace.xml',
        'server',
        {
          name: 'PortalApplication',
          type: 'gradle-boot',
          cwd: r,
          gradle: { project: ':apps:portal' },
          mainClass: 'com.example.portal.PortalApplication',
          spring: { profiles: ['local'] },
          runtime: { jdk: 'C:/Program Files/Java/jdk-17.0.15' },
          ports: [8080],
        },
        { command: 'gradlew.bat --console=plain -I sd-init.gradle :apps:portal:bootRun "--args=--spring.profiles.active=local --spring.output.ansi.enabled=always"' },
      ),
      mk(
        'intellij:.run/api-qa.run.xml#api-qa',
        'intellij',
        '.run/api-qa.run.xml',
        null,
        { name: 'api-qa', type: 'gradle-boot', cwd: r, gradle: { project: ':apps:api' }, args: ['--spring.profiles.active=qa'] },
        { command: 'gradlew.bat --console=plain :apps:api:bootRun "--args=--spring.profiles.active=qa"' },
      ),
      mk(
        'intellij:.run/npm-portal.run.xml#portal',
        'intellij',
        '.run/npm-portal.run.xml',
        'npm',
        { name: 'portal', type: 'npm', cwd: `${r}/front/portal`, npm: { script: 'dev' }, ports: [5173] },
        { command: 'npm.cmd run dev', scripts: ['build', 'dev', 'lint'] },
      ),
      mk(
        'intellij:.run/legacy.run.xml#legacy',
        'intellij',
        '.run/legacy.run.xml',
        null,
        { name: 'legacy-jar', type: 'jar', cwd: r, jar: { path: `${r}/dist/legacy.jar` } },
        { commandError: `jar 파일이 없습니다: ${r}/dist/legacy.jar`, warnings: ["대체 JRE '1.6'을(를) 찾지 못해 프로젝트 SDK를 씁니다."] },
      ),
      mk(
        'npm:front/admin',
        'npm',
        'front/admin/package.json',
        'npm',
        { name: 'admin-ui', type: 'npm', cwd: `${r}/front/admin`, npm: { script: 'serve', packageManager: 'yarn' } },
        { command: 'yarn.cmd serve', scripts: ['lint', 'serve'], recommended: false },
      ),
      mk(
        'gradle::apps:batch#com.example.batch.BatchApplication',
        'gradle',
        'Gradle :apps:batch',
        'server',
        {
          name: 'BatchApplication',
          type: 'gradle-boot',
          cwd: r,
          gradle: { project: ':apps:batch' },
          mainClass: 'com.example.batch.BatchApplication',
          spring: { profiles: ['local'] },
        },
        { command: 'gradlew.bat --console=plain :apps:batch:bootRun', profiles: ['local', 'prod'], recommended: false },
      ),
    ],
  }
}

export function createMockBackend(): Backend {
  let workspace: Workspace = structuredClone(sample)
  const statuses = new Map<string, ServiceStatus>()
  const logs = new Map<string, LogLine[]>()
  const seqs = new Map<string, number>()
  const timers = new Map<string, number[]>()
  const stateListeners = new Set<(s: ServiceStatus) => void>()
  const logListeners = new Set<(b: LogBatch[]) => void>()
  const errorListeners = new Set<(e: StartError) => void>()
  let runId = 1
  let settings: AppSettings = {
    currentWorkspace: 'default',
    closeToTray: true,
    notifications: true,
    logFiles: { enabled: false, retentionDays: 7 },
    clearConsoleOnStart: true,
    fonts: { ui: '', mono: '', consoleSize: 12.5 },
    autoUpdateCheck: true,
  }
  const progressListeners = new Set<(p: UpdateProgress) => void>()
  /** 직접 시작할 때 이전 로그 지우기 (실제 백엔드의 reset 묶음과 같게) */
  const resetLogs = (id: string) => {
    if (!settings.clearConsoleOnStart) return
    logs.set(id, [])
    logListeners.forEach((cb) => cb([{ id, reset: true, lines: [] }]))
  }
  const occupied = new Map<number, { pid: number; name: string }>([[5174, { pid: 4242, name: 'node.exe' }]])
  // 가짜 git 저장소 (ROOT 변수 폴더)
  const gitRoot = 'C:/Project/sample'
  let gitHead = 'develop'
  const gitLocals = [
    { name: 'develop', upstream: 'origin/develop' },
    { name: 'main', upstream: 'origin/main' },
    { name: 'feature/login-page', upstream: null as string | null },
  ]
  const gitRemoteRefs = ['origin/develop', 'origin/main', 'origin/feature/payment', 'origin/hotfix/1.2.1']
  const repoServices = () => workspace.services.filter((s) => (s.cwd ?? '').startsWith('${var:ROOT}')).map((s) => s.id)

  const emitState = (s: ServiceStatus) => {
    statuses.set(s.id, s)
    stateListeners.forEach((cb) => cb(s))
  }
  const push = (id: string, stream: number, text: string) => {
    const seq = (seqs.get(id) ?? 0) + 1
    seqs.set(id, seq)
    const line = { seq, ts: Date.now(), stream, text }
    const arr = logs.get(id) ?? []
    arr.push(line)
    logs.set(id, arr)
    logListeners.forEach((cb) => cb([{ id, lines: [line] }]))
  }
  const later = (id: string, ms: number, fn: () => void) => {
    const t = window.setTimeout(fn, ms)
    timers.set(id, [...(timers.get(id) ?? []), t])
  }
  const every = (id: string, ms: number, fn: () => void) => {
    const t = window.setInterval(fn, ms)
    timers.set(id, [...(timers.get(id) ?? []), t])
  }
  const clearTimers = (id: string) => {
    for (const t of timers.get(id) ?? []) {
      window.clearTimeout(t)
      window.clearInterval(t)
    }
    timers.delete(id)
  }

  const fakeCommand = (s: Service): string => {
    switch (s.type) {
      case 'npm':
        return `${s.npm?.packageManager ?? 'npm'}.cmd run ${s.npm?.script ?? ''}`
      case 'gradle-boot':
        return `gradlew.bat --console=plain ${s.gradle?.project ? s.gradle.project + ':' : ''}bootRun${
          s.spring?.profiles?.length ? ` "--args=--spring.profiles.active=${s.spring.profiles.join(',')}"` : ''
        }`
      case 'maven-boot':
        return `mvn.cmd ${s.maven?.module ? `-pl ${s.maven.module} ` : ''}spring-boot:run`
      case 'jar':
        return `java.exe ${joinArgs(s.jvmArgs)} -jar ${s.jar?.path ?? ''}`
      case 'shell':
        return `cmd.exe /d /s /c "${s.shell?.command ?? ''}"`
      default:
        return s.type
    }
  }

  /** 의존 대상을 먼저 시작하고 준비될 때까지 '대기 중'으로 둔다 */
  const launch = async (id: string, debug = false): Promise<void> => {
    const svc = workspace.services.find((s) => s.id === id)
    if (!svc) throw new Error(`서비스를 찾을 수 없습니다: ${id}`)
    const cur = statuses.get(id)?.state
    if (cur === 'running' || cur === 'starting' || cur === 'waiting') return
    const deps = (svc.dependsOn ?? []).filter((d) => statuses.get(d)?.state !== 'running')
    if (deps.length) {
      emitState({ ...(statuses.get(id) ?? blank(id)), state: 'waiting' })
      for (const d of deps) await launch(d)
      for (let i = 0; i < 100 && deps.some((d) => statuses.get(d)?.state !== 'running'); i++) await new Promise((r) => setTimeout(r, 100))
      if (statuses.get(id)?.state !== 'waiting') return
    }
    try {
      await start(id, debug)
    } catch (e) {
      const error = e instanceof Error ? e.message : String(e)
      emitState({ ...(statuses.get(id) ?? blank(id)), state: 'failed', error })
      errorListeners.forEach((cb) => cb({ id, name: svc.name, error }))
    }
  }

  const blank = (id: string): ServiceStatus => ({
    id, state: 'stopped', pid: null, exitCode: null, startedAt: null, endedAt: null, runId: 0, command: null, error: null, ports: [], urls: [], readyAt: null, debugPort: null,
  })

  const start = async (id: string, debug = false): Promise<ServiceStatus> => {
    const svc = workspace.services.find((s) => s.id === id)
    if (!svc) throw new Error(`서비스를 찾을 수 없습니다: ${id}`)
    if (statuses.get(id)?.state === 'running') throw new Error('이미 실행 중입니다.')
    const command = fakeCommand(svc)
    const conflict = (svc.ports ?? []).filter((p) => occupied.has(p))
    if (conflict.length) throw new Error(`포트가 이미 사용 중입니다: ${conflict.join(', ')}`)
    const status: ServiceStatus = {
      id,
      state: 'starting',
      pid: 10000 + Math.floor(Math.random() * 50000),
      exitCode: null,
      startedAt: Date.now(),
      endedAt: null,
      runId: runId++,
      command,
      error: null,
      ports: [],
      urls: [],
      readyAt: null,
      debugPort: debug ? (['npm', 'node'].includes(svc.type) ? 9229 : 5005) : null,
    }
    const ready = (ms: number, urls: string[]) =>
      later(id, ms, () => {
        const cur = statuses.get(id)
        if (cur?.state !== 'starting') return
        push(id, STREAM_SYS, `✓ 준비 완료 (${(ms / 1000).toFixed(1)}초)`)
        emitState({ ...cur, state: 'running', readyAt: Date.now(), urls, ports: urls.map((u) => Number(new URL(u).port)) })
      })
    push(id, STREAM_SYS, `▶ ${command}`)
    push(id, STREAM_SYS, `  작업 디렉터리: ${(svc.cwd ?? '~').replace('${var:ROOT}', 'C:/Project/sample')}`)
    emitState(status)

    if (svc.type === 'npm' || svc.type === 'node') {
      viteLines.forEach((l, i) => later(id, 150 + i * 40, () => push(id, STREAM_OUT, l)))
      ready(150 + 7 * 40 + 10, [`http://localhost:${svc.ports?.[0] ?? 5173}/`])
      every(id, 4000, () =>
        push(id, STREAM_OUT, `\x1b[2m${new Date().toLocaleTimeString()}\x1b[22m \x1b[36m\x1b[1m[vite]\x1b[22m\x1b[39m \x1b[32mhmr update \x1b[39m\x1b[2m/src/App.vue\x1b[22m`),
      )
    } else if (svc.type === 'gradle-boot' || svc.type === 'maven-boot' || svc.type === 'jar') {
      bootLines.forEach((l, i) => later(id, 300 + i * 30, () => push(id, STREAM_OUT, l)))
      const port = svc.ports?.[0] ?? 8080
      const boot: [number, Parameters<typeof bootLog>][] = [
        [900, ['INFO', 'k.e.portal.PortalApplication', 'Starting PortalApplication using Java 17.0.15 with PID 12345']],
        [950, ['INFO', 'k.e.portal.PortalApplication', 'The following 1 profile is active: "local"']],
        [1500, ['INFO', 'o.s.b.w.embedded.tomcat.TomcatWebServer', `Tomcat initialized with port ${port} (http)`]],
        [1700, ['WARN', 'o.s.b.a.orm.jpa.JpaBaseConfiguration', 'spring.jpa.open-in-view is enabled by default.']],
        [2300, ['INFO', 'o.s.b.w.embedded.tomcat.TomcatWebServer', `Tomcat started on port ${port} (http) with context path '/'`]],
        [2350, ['INFO', 'k.e.portal.PortalApplication', 'Started PortalApplication in 3.412 seconds (process running for 3.9)']],
      ]
      for (const [ms, args] of boot) later(id, ms, () => push(id, STREAM_OUT, bootLog(...args)))
      ready(2360, [`http://localhost:${port}/`])
      later(id, 2600, () => {
        push(id, STREAM_OUT, bootLog('ERROR', 'o.a.c.c.C.[.[.[/].[dispatcherServlet]', '요청 처리 중 오류가 발생했습니다 (한글 로그 예시)'))
        push(id, STREAM_ERR, 'java.lang.IllegalStateException: 샘플 예외')
        push(id, STREAM_ERR, '\tat com.example.portal.web.HomeController.index(HomeController.java:42)')
        push(id, STREAM_ERR, '\tat java.base/java.lang.Thread.run(Thread.java:840)')
      })
      every(id, 3000, () => push(id, STREAM_OUT, bootLog('DEBUG', 'k.e.portal.web.RequestLogFilter', `GET /api/health 200 ${Math.floor(Math.random() * 30)}ms`)))
    } else {
      emitState({ ...statuses.get(id)!, state: 'running', readyAt: Date.now() })
      later(id, 200, () => push(id, STREAM_OUT, '[+] Running 1/1'))
      later(id, 400, () => push(id, STREAM_OUT, ' \x1b[32m✔\x1b[0m Container redis  Started'))
      later(id, 3000, () => {
        clearTimers(id)
        push(id, STREAM_SYS, '✕ 비정상 종료 (종료 코드 1, 실행 시간 3.0초)')
        emitState({ ...status, state: 'failed', pid: null, exitCode: 1, endedAt: Date.now() })
      })
    }
    return status
  }

  const stop = async (id: string) => {
    const s = statuses.get(id)
    if (s?.state === 'waiting') {
      emitState({ ...s, state: 'stopped' })
      return
    }
    if (!s || (s.state !== 'running' && s.state !== 'starting')) return
    clearTimers(id)
    emitState({ ...s, state: 'stopping' })
    await new Promise((r) => setTimeout(r, 300))
    const elapsed = ((Date.now() - (s.startedAt ?? Date.now())) / 1000).toFixed(1)
    push(id, STREAM_SYS, `■ 중지됨 (실행 시간 ${elapsed}초)`)
    emitState({ ...s, state: 'stopped', pid: null, endedAt: Date.now() })
  }

  return {
    isMock: true,
    appInfo: async () => ({
      version: '0.1.10 (미리보기)',
      configDir: 'C:/Users/me/AppData/Roaming/ServicesDashboard',
      workspacePath: 'C:/Users/me/AppData/Roaming/ServicesDashboard/workspaces/default.json',
    }),
    getWorkspace: async () => ({ workspace: structuredClone(workspace), path: '(브라우저 미리보기)' }),
    reloadWorkspace: async () => ({ workspace: structuredClone(workspace), path: '(브라우저 미리보기)' }),
    saveWorkspace: async (ws) => {
      workspace = structuredClone(ws)
      return structuredClone(workspace)
    },
    statuses: async () => [...statuses.values()],
    start: (id, debug) => {
      if (!isActive(statuses.get(id)?.state)) resetLogs(id)
      void launch(id, debug)
      return Promise.resolve()
    },
    stop,
    restart: async (id, debug) => {
      await stop(id)
      resetLogs(id)
      void launch(id, debug)
    },
    startGroup: async (gid) => {
      for (const s of workspace.services.filter((s) => s.group === gid)) void launch(s.id)
    },
    startPreset: async (pid) => {
      for (const id of workspace.presets?.find((p) => p.id === pid)?.services ?? []) void launch(id)
    },
    stopPreset: async (pid) => {
      await Promise.all((workspace.presets?.find((p) => p.id === pid)?.services ?? []).map(stop))
    },
    projectExport: async (root) => `${root.replace(/[\\/]+$/, '')}/services-dashboard.json`,
    onStartError: async (cb) => {
      errorListeners.add(cb)
      return () => errorListeners.delete(cb)
    },
    stopGroup: async (gid) => {
      await Promise.all(workspace.services.filter((s) => s.group === gid).map((s) => stop(s.id)))
    },
    stopAll: async () => {
      await Promise.all([...statuses.keys()].map(stop))
    },
    getLogs: async (id) => [...(logs.get(id) ?? [])],
    clearLogs: async (id) => {
      logs.set(id, [])
    },
    preview: async (svc) => ({ command: fakeCommand(svc), cwd: (svc.cwd ?? '~').replace('${var:ROOT}', 'C:/Project/sample'), env: svc.env ?? {} }),
    portsCheck: async (id) => {
      const svc = workspace.services.find((s) => s.id === id)
      return (svc?.ports ?? []).filter((p) => occupied.has(p)).map((p) => ({ port: p, ...occupied.get(p)!, serviceId: null }))
    },
    processKill: async (pid) => {
      for (const [p, o] of occupied) if (o.pid === pid) occupied.delete(p)
    },
    settingsGet: async () => structuredClone(settings),
    settingsSave: async (next) => {
      settings = structuredClone(next)
      return structuredClone(settings)
    },
    // 브라우저에 흔히 있는 글꼴 (실제 목록은 Windows DirectWrite에서 읽는다)
    systemFonts: async () => [
      { name: 'Arial', mono: false },
      { name: 'Cascadia Mono', mono: true },
      { name: 'Consolas', mono: true },
      { name: 'Courier New', mono: true },
      { name: 'D2Coding', mono: true },
      { name: 'Gulim', local: '굴림', mono: false },
      { name: 'Malgun Gothic', local: '맑은 고딕', mono: false },
      { name: 'Segoe UI', mono: false },
      { name: 'Times New Roman', mono: false },
    ],
    logsExport: async (id) => (logs.get(id) ?? []).length,
    logsDir: async (id) => `C:/Users/me/AppData/Roaming/ServicesDashboard/logs/${id}`,
    pickSaveFile: async (name) => window.prompt('저장할 파일 경로 (미리보기)', `C:/temp/${name}`) || null,
    scanProject: async (root) => {
      await new Promise((r) => setTimeout(r, 300))
      if (!root.trim()) throw new Error('폴더를 입력하세요.')
      return mockScan(root)
    },
    detectRuntimes: async () => {
      await new Promise((r) => setTimeout(r, 200))
      return mockRuntimes
    },
    openPath: async (p) => console.info('[미리보기] 열기:', p),
    openUrl: async (u) => {
      window.open(u, '_blank')
    },
    pickFolder: async () => window.prompt('폴더 경로 (미리보기)') || null,
    pickFile: async () => window.prompt('파일 경로 (미리보기)') || null,
    confirm: async (message) => window.confirm(message),
    onState: async (cb) => {
      stateListeners.add(cb)
      return () => stateListeners.delete(cb)
    },
    onLogs: async (cb) => {
      logListeners.add(cb)
      return () => logListeners.delete(cb)
    },
    gitRepos: async () => [{ root: gitRoot, name: 'ROOT', branch: gitHead, commit: null, services: repoServices() }],
    gitBranches: async () => {
      await new Promise((r) => setTimeout(r, 200))
      return {
        root: gitRoot,
        head: { branch: gitHead, commit: null },
        locals: gitLocals.map((l) => ({ ...l })),
        remotes: gitRemoteRefs.filter((r) => !gitLocals.some((l) => l.upstream === r || r === `origin/${l.name}`)),
        changes: [],
        ahead: 0,
        behind: gitHead === 'develop' ? 2 : 0,
      }
    },
    gitFetch: () => new Promise((r) => setTimeout(r, 600)),
    gitSwitch: async (_root, branch, _pull, restart) => {
      const local = branch.startsWith('origin/') ? branch.slice('origin/'.length) : branch
      if (!gitLocals.some((l) => l.name === local)) {
        if (!gitRemoteRefs.includes(branch)) throw new Error(`브랜치를 찾을 수 없습니다: ${branch}`)
        gitLocals.unshift({ name: local, upstream: branch })
      }
      const ids = repoServices().filter((id) => isActive(statuses.get(id)?.state))
      for (const id of ids) {
        push(id, STREAM_SYS, `⎇ 브랜치 전환: ${gitHead} → ${branch}`)
        await stop(id)
      }
      gitHead = local
      if (restart) for (const id of ids) void launch(id)
      return { branch: local, stopped: ids, restarted: restart && ids.length > 0, warnings: [] }
    },
    // 미리보기: 늘 새 버전이 있는 것처럼 보이고, 받기는 흉내만 내고, 설치는 하지 않는다
    updateCheck: async () => {
      await new Promise((r) => setTimeout(r, 500))
      return { version: '0.2.0', currentVersion: '0.1.10', notes: '미리보기용 가짜 업데이트입니다.' }
    },
    updateDownload: async () => {
      const total = 2_700_000
      for (let done = 0; done < total; done += 270_000) {
        for (const cb of progressListeners) cb({ downloaded: done, total })
        await new Promise((r) => setTimeout(r, 120))
      }
      for (const cb of progressListeners) cb({ downloaded: total, total })
    },
    updateInstall: async () => {
      throw new Error('브라우저 미리보기에서는 설치하지 않습니다.')
    },
    onUpdateProgress: async (cb) => {
      progressListeners.add(cb)
      return () => progressListeners.delete(cb)
    },
  }
}
