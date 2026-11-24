// 실제 앱 E2E 공통 하네스.
//
// 빌드한 앱을 따로 띄워 WebView2 원격 디버깅(CDP)으로 화면을 읽고, 실제 마우스·키 입력을 보낸다.
// - 설정 폴더(SERVICES_DASHBOARD_HOME)와 WebView2 데이터 폴더는 OS 임시 폴더 아래에 매번 새로 만든다.
//   그래서 평소 쓰는 설치본과 섞이지 않고, 지난 실행 상태에 기대지 않는다.
// - 시험이 끄는 프로세스는 자기가 띄운 앱(PID)뿐이다. 설치본은 건드리지 않는다.
// - 시험 창은 띄우자마자 최소화한다. WebView2는 운영체제 마우스 메시지를 받으면 마우스 위치를 실제 커서로
//   다시 맞추므로, 사용자 커서가 창 위에서 움직이면 hover 시험이 깨진다. 최소화해도 페이지는 계속 그려지고
//   CDP 입력도 그대로 동작한다. 창을 보면서 시험하려면 SD_SHOW=1.
import fs from 'node:fs'
import net from 'node:net'
import os from 'node:os'
import path from 'node:path'
import { execFileSync, spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'

export const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
export const RELEASE_EXE = path.join(REPO, 'src-tauri', 'target', 'release', 'services-dashboard.exe')
export const DEBUG_EXE = path.join(REPO, 'src-tauri', 'target', 'debug', 'services-dashboard.exe')
/** 시험을 건너뛸 때의 종료 코드 (run-all이 '건너뜀'으로 센다) */
export const EXIT_SKIP = 77

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
/** Windows 경로를 JSON 설정에 넣기 좋게 / 로 */
export const slash = (p) => p.split(path.sep).join('/')

const freePort = () =>
  new Promise((resolve, reject) => {
    const s = net.createServer()
    s.unref()
    s.on('error', reject)
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address()
      s.close(() => resolve(port))
    })
  })

function hideWindow(pid) {
  if (process.env.SD_SHOW) return
  const script = path.join(REPO, 'e2e', 'win.ps1')
  for (const shell of ['pwsh', 'powershell']) {
    try {
      const out = execFileSync(shell, ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', script, String(pid)]).toString().trim()
      if (!out.includes('iconic=True')) console.log(`    (시험 창 최소화 실패: ${out})`)
      return
    } catch (e) {
      if (e.code === 'ENOENT') continue
      console.log(`    (시험 창 최소화 실패: ${String(e.message).split(/\r?\n/)[0]})`)
      return
    }
  }
}

/** Ctrl+A, Alt+ArrowUp, Escape 같은 이름을 CDP 키 입력으로 */
const NAMED_KEYS = { Escape: 27, Enter: 13, Tab: 9, ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39, Home: 36, End: 35, Delete: 46 }
const MODIFIERS = { Alt: 1, Ctrl: 2, Meta: 4, Shift: 8 }
function parseCombo(combo) {
  const parts = combo.split('+')
  const name = parts.pop()
  let modifiers = 0
  for (const m of parts) {
    if (!(m in MODIFIERS)) throw new Error(`모르는 보조 키: ${m}`)
    modifiers |= MODIFIERS[m]
  }
  if (name.length === 1) {
    const up = name.toUpperCase()
    return { modifiers, key: name.toLowerCase(), code: /[A-Z]/.test(up) ? `Key${up}` : `Digit${up}`, vk: up.charCodeAt(0) }
  }
  if (!(name in NAMED_KEYS)) throw new Error(`모르는 키: ${name}`)
  return { modifiers, key: name, code: name, vk: NAMED_KEYS[name] }
}

/** CDP로 조종하는 앱 하나 */
class App {
  constructor(suite, proc, ws) {
    this.suite = suite
    this.proc = proc
    this.pid = proc.pid
    this.ws = ws
    this.seq = 0
    this.pending = new Map()
    this.listeners = new Map()
    this.exited = new Promise((r) => proc.once('exit', r))
    this.mouseAt = { x: 600, y: 400 }
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data)
      if (m.id && this.pending.has(m.id)) {
        this.pending.get(m.id)(m)
        this.pending.delete(m.id)
      } else if (m.method) {
        for (const cb of this.listeners.get(m.method) ?? []) cb(m.params)
      }
    }
  }

  /** CDP 이벤트 받기 (예: Input.dragIntercepted) */
  on(method, cb) {
    if (!this.listeners.has(method)) this.listeners.set(method, [])
    this.listeners.get(method).push(cb)
  }

  send(method, params = {}) {
    return new Promise((resolve) => {
      const id = ++this.seq
      this.pending.set(id, resolve)
      this.ws.send(JSON.stringify({ id, method, params }))
    })
  }

  /** 페이지에서 식을 계산해 값을 돌려준다 (Promise면 기다림) */
  async evaluate(expression) {
    const r = await this.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (r.error) throw new Error(`CDP 오류: ${JSON.stringify(r.error)}`)
    if (r.result?.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails).slice(0, 400))
    return r.result?.result?.value
  }

  /** 백엔드 명령 호출 (실패하면 예외) */
  invoke(cmd, args = {}) {
    return this.evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)})`)
  }

  /** 백엔드 명령 호출 → { ok, v } 또는 { ok: false, e } */
  tryInvoke(cmd, args = {}) {
    return this.evaluate(
      `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)}).then(v => ({ ok: true, v }), e => ({ ok: false, e: String(e) }))`,
    )
  }

  /** fn이 참이 될 때까지 기다린다 */
  async waitFor(fn, ms = 10000, step = 150) {
    const end = Date.now() + ms
    while (Date.now() < end) {
      if (await fn()) return true
      await sleep(step)
    }
    return false
  }

  /** waitFor에 넘길 화면 조건 */
  ui(expression) {
    return () => this.evaluate(expression)
  }

  async shot(name) {
    const r = await this.send('Page.captureScreenshot', { format: 'png' })
    if (!r.result?.data) return
    const dir = path.join(this.suite.dir, 'shots')
    fs.mkdirSync(dir, { recursive: true })
    fs.writeFileSync(path.join(dir, name), Buffer.from(r.result.data, 'base64'))
  }

  /** 선택자의 첫 요소 위치 (없으면 null) */
  rect(sel, index = 0) {
    return this.evaluate(`(() => {
      const e = document.querySelectorAll(${JSON.stringify(sel)})[${index}]
      if (!e) return null
      const b = e.getBoundingClientRect()
      return { l: b.left, t: b.top, w: b.width, h: b.height, r: b.right, b: b.bottom }
    })()`)
  }

  /** 마우스 이벤트 하나 (buttons: 누르고 있는 버튼, 1 = 왼쪽) */
  mouse(type, x, y, buttons = 0) {
    if (type === 'mouseMoved') this.mouseAt = { x, y }
    return this.send('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons, clickCount: 1 })
  }

  /** 지금 위치에서 (x, y)까지 여러 걸음으로 옮긴다 (중간 요소의 mouseenter·mouseleave가 생기게) */
  async moveTo(x, y, steps = 6, buttons = 0) {
    const { x: x0, y: y0 } = this.mouseAt
    for (let k = 1; k <= steps; k++) {
      await this.mouse('mouseMoved', x0 + ((x - x0) * k) / steps, y0 + ((y - y0) * k) / steps, buttons)
      await sleep(15)
    }
  }

  /** 한 번에 옮긴다 (점프) */
  jumpTo(x, y) {
    return this.mouse('mouseMoved', x, y)
  }

  async clickAt(x, y) {
    await this.moveTo(x, y)
    await this.mouse('mousePressed', x, y, 1)
    await this.mouse('mouseReleased', x, y)
  }

  /** 요소 가운데(키 큰 요소는 위쪽)를 실제 마우스로 누른다 */
  async click(sel, maxDy = 60) {
    const r = await this.rect(sel)
    if (!r) throw new Error(`요소 없음: ${sel}`)
    await this.clickAt(r.l + r.w / 2, r.t + Math.min(r.h / 2, maxDy))
  }

  /** 키 조합 누르기: 'Ctrl+A', 'Alt+ArrowUp', 'Escape' */
  async press(combo) {
    const { modifiers, key, code, vk } = parseCombo(combo)
    const base = { modifiers, key, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk }
    await this.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', ...base })
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', ...base })
  }

  /** 입력칸에 글자 넣기 (IME 없이) */
  type(text) {
    return this.send('Input.insertText', { text })
  }

  // ── 자주 쓰는 화면 조작 ──

  /** 서비스 트리에서 이름이 같은 행을 누른다 */
  clickRow(name) {
    return this.evaluate(`(() => {
      const r = [...document.querySelectorAll('.list .row')].find(r => r.querySelector('.name')?.textContent.trim() === ${JSON.stringify(name)})
      r?.click()
      return !!r
    })()`)
  }

  /** 글자가 들어간 버튼을 누른다 (scope 선택자 안에서) */
  clickButton(text, scope = 'button') {
    return this.evaluate(`(() => {
      const b = [...document.querySelectorAll(${JSON.stringify(scope)})].find(b => b.textContent.includes(${JSON.stringify(text)}))
      b?.click()
      return !!b
    })()`)
  }

  /** 도구 모음에서 설명(title)이 이 글자로 시작하는 버튼을 누른다 */
  clickTool(title) {
    return this.evaluate(`(() => {
      const b = [...document.querySelectorAll('.toolbar .tool')].find(b => b.title.startsWith(${JSON.stringify(title)}))
      b?.click()
      return !!b
    })()`)
  }

  async statusOf(id) {
    return (await this.invoke('service_statuses')).find((s) => s.id === id) ?? { state: 'stopped' }
  }

  async logTexts(id) {
    return (await this.invoke('logs_get', { id })).map((l) => l.text)
  }

  /** 서비스를 모두 멈추고 앱을 끈다 */
  async quit() {
    if (this.proc.exitCode === null) {
      try {
        await Promise.race([this.invoke('stop_all'), sleep(8000)])
      } catch {}
    }
    this.kill()
    await Promise.race([this.exited, sleep(5000)])
    this.suite.apps.delete(this)
    // WebView2 하위 프로세스가 데이터 폴더를 놓을 때까지 잠깐
    await sleep(800)
  }

  kill() {
    try {
      this.ws.close()
    } catch {}
    if (this.proc.exitCode === null) {
      try {
        process.kill(this.pid)
      } catch {}
    }
  }
}

/**
 * 시험 하나. 작업 폴더를 만들고 결과를 모은다.
 * @param {string} name 시험 이름 (작업 폴더 이름에 쓰임)
 */
export function createSuite(name) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `sd-e2e-${name}-`))
  const home = path.join(dir, 'home')
  fs.mkdirSync(path.join(home, 'workspaces'), { recursive: true })
  const results = []
  const apps = new Set()

  const killAll = () => {
    for (const a of apps) a.kill()
  }
  process.on('exit', killAll)
  const fail = (e) => {
    console.error(`\n오류: ${e?.stack ?? e}`)
    console.error(`작업 폴더(스크린샷 포함)를 남겼습니다: ${dir}`)
    killAll()
    process.exit(1)
  }
  process.on('uncaughtException', fail)
  process.on('unhandledRejection', fail)

  const suite = {
    name,
    dir,
    home,
    apps,

    /** 이 시험 폴더 안의 경로 */
    path: (...p) => path.join(dir, ...p),

    /** 설정 파일을 새로 쓴다. 자동 업데이트 확인은 기본으로 끈다 (GitHub에 요청하지 않게). */
    writeSettings(settings = {}) {
      const s = { currentWorkspace: name, closeToTray: false, notifications: false, autoUpdateCheck: false, ...settings }
      fs.writeFileSync(path.join(home, 'settings.json'), JSON.stringify(s, null, 2))
    },
    readSettings() {
      return JSON.parse(fs.readFileSync(path.join(home, 'settings.json'), 'utf8'))
    },
    /** 작업공간 파일을 새로 쓴다 (id = 파일 이름) */
    writeWorkspace(workspace, id = name) {
      fs.writeFileSync(path.join(home, 'workspaces', `${id}.json`), JSON.stringify({ name: id, groups: [], services: [], ...workspace }, null, 2))
    },
    readWorkspace(id = name) {
      return JSON.parse(fs.readFileSync(path.join(home, 'workspaces', `${id}.json`), 'utf8'))
    },
    /**
     * 시험용 Node 서비스(server.js)를 만들고 그 폴더를 돌려준다.
     * 300줄 + 오류 줄 + 스택트레이스 8줄(일부는 콘솔에서 접힘) + READY-MARK를 찍고 계속 돈다.
     */
    writeSampleApp() {
      const appDir = path.join(dir, 'app')
      fs.mkdirSync(appDir, { recursive: true })
      fs.writeFileSync(
        path.join(appDir, 'server.js'),
        [
          "for (let i = 1; i <= 300; i++) console.log('line ' + i)",
          "console.log('2026-01-01 ERROR 처리 실패')",
          "console.log('java.lang.IllegalStateException: 샘플')",
          "for (let i = 1; i <= 8; i++) console.log('\\tat a.B.m' + i + '(B.java:' + i + ')')",
          "console.log('READY-MARK')",
          'setInterval(() => {}, 1000)',
        ].join('\n'),
      )
      return slash(appDir)
    },

    check(label, ok, detail = '') {
      results.push({ label, ok: !!ok })
      console.log(`${ok ? 'PASS' : 'FAIL'}  ${label}${detail ? `  — ${detail}` : ''}`)
      return !!ok
    },

    /** 앱을 띄우고 CDP로 연결한다 */
    async launch({ exe = process.env.SD_EXE || RELEASE_EXE, env = {} } = {}) {
      if (!fs.existsSync(exe)) throw new Error(`앱 실행 파일이 없습니다: ${exe}\n먼저 빌드하세요 (e2e/README.md).`)
      const port = await freePort()
      const proc = spawn(exe, [], {
        env: {
          ...process.env,
          SERVICES_DASHBOARD_HOME: home,
          WEBVIEW2_USER_DATA_FOLDER: path.join(dir, 'webview'),
          WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
          ...env,
        },
        detached: true,
        stdio: 'ignore',
      })
      let target
      for (let i = 0; i < 80 && !target && proc.exitCode === null; i++) {
        try {
          target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === 'page')
        } catch {}
        if (!target) await sleep(500)
      }
      if (!target) {
        try {
          process.kill(proc.pid)
        } catch {}
        throw new Error(`앱 화면에 연결하지 못했습니다 (포트 ${port}, 종료 코드 ${proc.exitCode})`)
      }
      const ws = new WebSocket(target.webSocketDebuggerUrl)
      await new Promise((resolve, reject) => {
        ws.onopen = resolve
        ws.onerror = reject
      })
      const app = new App(suite, proc, ws)
      apps.add(app)
      hideWindow(proc.pid)
      return app
    },

    /** 이 시험을 건너뛴다 */
    skip(reason) {
      console.log(`SKIP  ${reason}`)
      killAll()
      fs.rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 300 })
      process.exit(EXIT_SKIP)
    },

    /** 결과를 출력하고 끝낸다. 모두 통과하면 작업 폴더를 지우고(SD_KEEP=1이면 남김), 실패하면 남긴다. */
    async finish() {
      for (const a of [...apps]) await a.quit()
      const failed = results.filter((r) => !r.ok)
      console.log(`\n결과: ${results.length - failed.length}/${results.length} 통과`)
      if (failed.length || process.env.SD_KEEP) {
        console.log(`작업 폴더(스크린샷 포함): ${dir}`)
      } else {
        try {
          fs.rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 })
        } catch {
          console.log(`(작업 폴더를 지우지 못했습니다: ${dir})`)
        }
      }
      process.exit(failed.length ? 1 : 0)
    },
  }
  return suite
}
