// 브라우저 미리보기용 가짜 백엔드.
// `npm run dev`로 띄운 화면을 일반 브라우저에서 열면 이 백엔드가 쓰인다 (실제 프로세스는 실행하지 않음).

import type { Backend } from './api'
import { joinArgs } from './args'
import type { Candidate, LogBatch, LogLine, RuntimeInfo, ScanResult, Service, ServiceStatus, Workspace } from './types'
import { STREAM_ERR, STREAM_OUT, STREAM_SYS } from './types'

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
    },
    { id: 'auth-api', name: 'AuthApplication', group: 'server', type: 'maven-boot', cwd: '${var:ROOT}/auth', maven: { module: 'auth-api' }, ports: [8081] },
    { id: 'batch', name: 'batch-worker', type: 'shell', shell: { command: 'docker compose up redis', shell: 'cmd' } },
  ],
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
    warnings: ['가져오지 않은 IntelliJ 구성: all (복합 구성은 M4 실행 프리셋에서 지원 예정)'],
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
  let runId = 1

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

  const start = async (id: string): Promise<ServiceStatus> => {
    const svc = workspace.services.find((s) => s.id === id)
    if (!svc) throw new Error(`서비스를 찾을 수 없습니다: ${id}`)
    if (statuses.get(id)?.state === 'running') throw new Error('이미 실행 중입니다.')
    const command = fakeCommand(svc)
    const status: ServiceStatus = {
      id,
      state: 'running',
      pid: 10000 + Math.floor(Math.random() * 50000),
      exitCode: null,
      startedAt: Date.now(),
      endedAt: null,
      runId: runId++,
      command,
      error: null,
    }
    push(id, STREAM_SYS, `▶ ${command}`)
    push(id, STREAM_SYS, `  작업 디렉터리: ${(svc.cwd ?? '~').replace('${var:ROOT}', 'C:/Project/sample')}`)
    emitState(status)

    if (svc.type === 'npm' || svc.type === 'node') {
      viteLines.forEach((l, i) => later(id, 150 + i * 40, () => push(id, STREAM_OUT, l)))
      every(id, 4000, () =>
        push(id, STREAM_OUT, `\x1b[2m${new Date().toLocaleTimeString()}\x1b[22m \x1b[36m\x1b[1m[vite]\x1b[22m\x1b[39m \x1b[32mhmr update \x1b[39m\x1b[2m/src/App.vue\x1b[22m`),
      )
    } else if (svc.type === 'gradle-boot' || svc.type === 'maven-boot' || svc.type === 'jar') {
      bootLines.forEach((l, i) => later(id, 300 + i * 30, () => push(id, STREAM_OUT, l)))
      const boot: [number, Parameters<typeof bootLog>][] = [
        [900, ['INFO', 'k.e.portal.PortalApplication', 'Starting PortalApplication using Java 17.0.15 with PID 12345']],
        [950, ['INFO', 'k.e.portal.PortalApplication', 'The following 1 profile is active: "local"']],
        [1500, ['INFO', 'o.s.b.w.embedded.tomcat.TomcatWebServer', 'Tomcat initialized with port 8080 (http)']],
        [1700, ['WARN', 'o.s.b.a.orm.jpa.JpaBaseConfiguration', 'spring.jpa.open-in-view is enabled by default.']],
        [2300, ['INFO', 'o.s.b.w.embedded.tomcat.TomcatWebServer', 'Tomcat started on port 8080 (http) with context path \'/\'']],
        [2350, ['INFO', 'k.e.portal.PortalApplication', 'Started PortalApplication in 3.412 seconds (process running for 3.9)']],
      ]
      for (const [ms, args] of boot) later(id, ms, () => push(id, STREAM_OUT, bootLog(...args)))
      later(id, 2600, () => {
        push(id, STREAM_OUT, bootLog('ERROR', 'o.a.c.c.C.[.[.[/].[dispatcherServlet]', '요청 처리 중 오류가 발생했습니다 (한글 로그 예시)'))
        push(id, STREAM_ERR, 'java.lang.IllegalStateException: 샘플 예외')
        push(id, STREAM_ERR, '\tat com.example.portal.web.HomeController.index(HomeController.java:42)')
        push(id, STREAM_ERR, '\tat java.base/java.lang.Thread.run(Thread.java:840)')
      })
      every(id, 3000, () => push(id, STREAM_OUT, bootLog('DEBUG', 'k.e.portal.web.RequestLogFilter', `GET /api/health 200 ${Math.floor(Math.random() * 30)}ms`)))
    } else {
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
    if (!s || s.state !== 'running') return
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
      version: '0.1.0 (미리보기)',
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
    start,
    stop,
    restart: async (id) => {
      await stop(id)
      return start(id)
    },
    startGroup: async (gid) => {
      for (const s of workspace.services.filter((s) => s.group === gid)) {
        if (statuses.get(s.id)?.state !== 'running') await start(s.id)
      }
      return []
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
  }
}
