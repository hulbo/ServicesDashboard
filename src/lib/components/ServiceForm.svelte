<script lang="ts">
  import { untrack } from 'svelte'
  import { app } from '../state.svelte'
  import { errorMessage } from '../api'
  import { joinArgs, parsePorts, splitArgs, splitLines, splitList } from '../args'
  import { cleanForSave, normalizeForEdit, usesArgs, usesColorInject, usesJvmArgs, usesUtf8Inject } from '../service-edit'
  import type { EnvMap, Service, ServiceKind } from '../types'
  import { DEFAULT_READY_HINT, JAVA_KINDS, KIND_LABELS, MAIN_CLASS_KINDS, MAVEN_KINDS, NODE_KINDS, SOURCE_LABELS, SPRING_KINDS, isActive } from '../types'
  import Icon from './Icon.svelte'
  import KeyValueEditor from './KeyValueEditor.svelte'
  import RuntimeSelect from './RuntimeSelect.svelte'

  let { service }: { service: Service } = $props()

  const snapshot = (s: Service) => $state.snapshot(s) as Service
  const keyOf = (s: Service) => JSON.stringify(cleanForSave(snapshot(s)))

  // 초기값만 쓰고, 이후 변경은 아래 $effect.pre에서 반영한다.
  let draft = $state<Service>(untrack(() => normalizeForEdit(snapshot(service))))
  let loadedId = $state(untrack(() => service.id))
  let loadedKey = $state(untrack(() => keyOf(service)))
  let saving = $state(false)
  /** 초안을 통째로 바꿀 때마다 올려서 KeyValueEditor 입력 행을 다시 채우게 한다. */
  let formRev = $state(0)

  const dirty = $derived(keyOf(draft) !== keyOf(service))
  const running = $derived(isActive(app.statuses[service.id]?.state))
  const kind = $derived(draft.type)

  // 편집 대상이 바뀌거나, 저장·이름 변경 등으로 원본이 바뀌었고 편집 중이 아니면 다시 불러온다.
  $effect.pre(() => {
    const key = keyOf(service)
    if (service.id !== loadedId || (key !== loadedKey && !dirtyAgainst(loadedKey))) {
      draft = normalizeForEdit(snapshot(service))
      loadedId = service.id
      untrack(() => formRev++)
    }
    loadedKey = key
  })

  function dirtyAgainst(key: string) {
    return keyOf(draft) !== key
  }

  // 실행 명령 미리보기
  let preview = $state<{ command?: string; cwd?: string; env?: EnvMap; error?: string } | null>(null)
  let previewSeq = 0
  $effect(() => {
    const s = cleanForSave(snapshot(draft))
    const seq = ++previewSeq
    const t = setTimeout(async () => {
      try {
        const p = await app.backend.preview(s)
        if (seq === previewSeq) preview = { command: p.command, cwd: p.cwd, env: p.env }
      } catch (e) {
        if (seq === previewSeq) preview = { error: errorMessage(e) }
      }
    }, 300)
    return () => clearTimeout(t)
  })

  async function save(restart = false) {
    if (saving) return
    saving = true
    try {
      const cleaned = cleanForSave(snapshot(draft))
      const ok = await app.updateService(cleaned)
      if (ok) {
        draft = normalizeForEdit(cleaned)
        if (restart) await app.restart(service.id)
        else app.toast('success', '저장했습니다.')
      }
    } finally {
      saving = false
    }
  }

  function revert() {
    draft = normalizeForEdit(snapshot(service))
    formRev++
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.ctrlKey && (e.key === 's' || e.key === 'S')) {
      e.preventDefault()
      if (dirty) void save(false)
    }
  }

  async function browseFolder(current: string | undefined, apply: (p: string) => void) {
    const p = await app.backend.pickFolder(current && !current.includes('${') ? current : undefined)
    if (p) apply(p)
  }

  async function browseFile(current: string | undefined, exts: string[], apply: (p: string) => void) {
    const p = await app.backend.pickFile(current && !current.includes('${') ? current : undefined, exts)
    if (p) apply(p)
  }

  function setKind(k: ServiceKind) {
    draft.type = k
  }

  const kinds = Object.keys(KIND_LABELS) as ServiceKind[]

  /** 미리보기에 보여 줄 주요 환경변수 (PATH처럼 긴 값은 뺀다) */
  const previewEnv = $derived.by(() => {
    const env = preview?.env ?? {}
    const rows: [string, string][] = []
    if (env.SD_JVM_ARGS) rows.push(['VM 옵션 (init script로 주입)', env.SD_JVM_ARGS.split('\u001f').join(' ')])
    if (env.SD_MAIN_CLASS) rows.push(['메인 클래스 (init script로 주입)', env.SD_MAIN_CLASS])
    if (env.JAVA_HOME) rows.push(['JAVA_HOME', env.JAVA_HOME])
    if (env.FORCE_COLOR) rows.push(['FORCE_COLOR', env.FORCE_COLOR])
    return rows
  })

  function setInject(key: 'utf8' | 'color', on: boolean) {
    draft.inject = { ...(draft.inject ?? {}), [key]: on }
  }

  const argsHelp = $derived.by(() => {
    switch (kind) {
      case 'npm':
        return { label: '스크립트 추가 인자', placeholder: '--port 5174', hint: 'npm은 -- 뒤에 붙여 스크립트로 전달합니다.' }
      case 'gradle-boot':
      case 'maven-boot':
      case 'jar':
        return { label: '프로그램 인자', placeholder: '--server.port=8081 --logging.level.root=info', hint: 'Spring Boot 인자. 설정값 덮어쓰기에 씁니다.' }
      case 'gradle':
        return { label: '--args 값', placeholder: '--foo bar', hint: 'run 같은 JavaExec 태스크에 --args로 전달합니다.' }
      default:
        return { label: '프로그램 인자', placeholder: '', hint: '' }
    }
  })
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<form class="form" onsubmit={(e) => { e.preventDefault(); void save(false) }} onkeydown={onKeydown}>
  <div class="scroll">
    {#if draft.source}
      <div class="source">
        <Icon name="import" size={14} />
        <span>
          가져온 출처: <b>{SOURCE_LABELS[draft.source.kind] ?? draft.source.kind}</b> · <span class="mono">{draft.source.origin}</span>
          <span class="muted mono">({draft.source.root})</span>
        </span>
        <span class="spacer"></span>
        <button type="button" class="link-btn" disabled={dirty} title={dirty ? '저장하지 않은 변경 사항이 있습니다' : '원본 구성을 다시 읽어 실행 정의를 갱신합니다'} onclick={() => app.resync(service.id)}>
          <Icon name="refresh" size={13} /> 원본에서 다시 가져오기
        </button>
      </div>
    {/if}
    <section>
      <h3>기본</h3>
      <div class="grid">
        <label for="f-name">이름</label>
        <input id="f-name" bind:value={draft.name} spellcheck="false" />

        <label for="f-group">그룹</label>
        <select id="f-group" bind:value={() => draft.group ?? '', (v) => (draft.group = v || undefined)}>
          <option value="">(그룹 없음)</option>
          {#each app.workspace.groups as g (g.id)}
            <option value={g.id}>{g.name}</option>
          {/each}
        </select>

        <label for="f-type">실행 타입</label>
        <select id="f-type" value={draft.type} onchange={(e) => setKind(e.currentTarget.value as ServiceKind)}>
          {#each kinds as k (k)}
            <option value={k}>{KIND_LABELS[k]}</option>
          {/each}
        </select>

        <label for="f-cwd">작업 디렉터리</label>
        <div class="with-btn">
          <input id="f-cwd" bind:value={() => draft.cwd ?? '', (v) => (draft.cwd = v)} placeholder={kind === 'shell' ? '비우면 사용자 홈' : '${var:ROOT}/front/portal'} spellcheck="false" class="mono" />
          <button type="button" class="btn-icon" title="폴더 선택" onclick={() => browseFolder(draft.cwd, (p) => (draft.cwd = p))}><Icon name="folder" size={14} /></button>
        </div>
      </div>
    </section>

    <section>
      <h3>실행</h3>
      <div class="grid">
        {#if kind === 'npm' && draft.npm}
          <label for="f-script">스크립트</label>
          <input id="f-script" bind:value={draft.npm.script} placeholder="dev" spellcheck="false" class="mono" />
          <label for="f-pm">패키지 매니저</label>
          <select id="f-pm" bind:value={draft.npm.packageManager}>
            <option value="npm">npm</option>
            <option value="yarn">yarn</option>
            <option value="pnpm">pnpm</option>
          </select>
        {:else if kind === 'node' && draft.node}
          <label for="f-entry">스크립트 파일</label>
          <input id="f-entry" bind:value={draft.node.entry} placeholder="server.js" spellcheck="false" class="mono" />
          <label for="f-nodeargs">Node 옵션</label>
          <input id="f-nodeargs" value={joinArgs(draft.node.nodeArgs)} onchange={(e) => draft.node && (draft.node.nodeArgs = splitArgs(e.currentTarget.value))} placeholder="--max-old-space-size=4096" spellcheck="false" class="mono" />
        {:else if (kind === 'gradle-boot' || kind === 'gradle') && draft.gradle}
          <label for="f-gproj">Gradle 프로젝트</label>
          <div>
            <input id="f-gproj" bind:value={() => draft.gradle?.project ?? '', (v) => draft.gradle && (draft.gradle.project = v)} placeholder=":apps:portal (비우면 루트 프로젝트)" spellcheck="false" class="mono" />
          </div>
          {#if kind === 'gradle'}
            <label for="f-gtasks">태스크</label>
            <input id="f-gtasks" value={joinArgs(draft.gradle.tasks)} onchange={(e) => draft.gradle && (draft.gradle.tasks = splitArgs(e.currentTarget.value))} placeholder="run" spellcheck="false" class="mono" />
          {/if}
          <label for="f-gopts">추가 Gradle 옵션</label>
          <input id="f-gopts" value={joinArgs(draft.gradle.options)} onchange={(e) => draft.gradle && (draft.gradle.options = splitArgs(e.currentTarget.value))} placeholder="--offline" spellcheck="false" class="mono" />
          <span class="label">Gradle 데몬</span>
          <div>
            <label class="check">
              <input type="checkbox" checked={draft.gradle.daemon === true} onchange={(e) => draft.gradle && (draft.gradle.daemon = e.currentTarget.checked)} />
              데몬 재사용 (시작이 빨라짐)
            </label>
            <p class="hint">끄면 --no-daemon으로 실행해 중지할 때 빌드 JVM과 앱이 바로 함께 종료됩니다. 켜면 IntelliJ 등이 띄운 데몬에 붙을 수 있고, 그때는 중지 후 앱 종료가 몇 초 늦을 수 있습니다.</p>
          </div>
        {:else if (kind === 'maven-boot' || kind === 'maven') && draft.maven}
          <label for="f-mmod">모듈 (-pl)</label>
          <input id="f-mmod" bind:value={() => draft.maven?.module ?? '', (v) => draft.maven && (draft.maven.module = v)} placeholder="portal (비우면 루트)" spellcheck="false" class="mono" />
          {#if kind === 'maven'}
            <label for="f-mgoals">골</label>
            <input id="f-mgoals" value={joinArgs(draft.maven.goals)} onchange={(e) => draft.maven && (draft.maven.goals = splitArgs(e.currentTarget.value))} placeholder="exec:java" spellcheck="false" class="mono" />
          {/if}
          <label for="f-mopts">추가 Maven 옵션</label>
          <input id="f-mopts" value={joinArgs(draft.maven.options)} onchange={(e) => draft.maven && (draft.maven.options = splitArgs(e.currentTarget.value))} placeholder="-o -q" spellcheck="false" class="mono" />
        {:else if kind === 'jar' && draft.jar}
          <label for="f-jar">jar 파일</label>
          <div class="with-btn">
            <input id="f-jar" bind:value={draft.jar.path} placeholder="build/libs/app.jar" spellcheck="false" class="mono" />
            <button type="button" class="btn-icon" title="파일 선택" onclick={() => browseFile(draft.jar?.path, ['jar'], (p) => draft.jar && (draft.jar.path = p))}><Icon name="file" size={14} /></button>
          </div>
        {:else if kind === 'shell' && draft.shell}
          <label for="f-shell">셸</label>
          <select id="f-shell" bind:value={draft.shell.shell}>
            <option value="cmd">cmd</option>
            <option value="powershell">Windows PowerShell</option>
            <option value="pwsh">PowerShell 7 (pwsh)</option>
            <option value="bash">bash (Git Bash 등)</option>
          </select>
          <label for="f-cmd">명령</label>
          <textarea id="f-cmd" rows="3" bind:value={draft.shell.command} placeholder="docker compose up redis" spellcheck="false" class="mono"></textarea>
        {:else if kind === 'java-main'}
          <span></span>
          <p class="notice">java-main(메인 클래스 직접 실행)은 M4에서 지원할 예정입니다. 지금은 Spring Boot (Gradle/Maven) 타입을 사용하세요.</p>
        {/if}

        {#if MAIN_CLASS_KINDS.includes(kind)}
          <label for="f-main">메인 클래스</label>
          <div>
            <input id="f-main" bind:value={() => draft.mainClass ?? '', (v) => (draft.mainClass = v)} placeholder="com.example.Application (비우면 자동 감지)" spellcheck="false" class="mono" />
            <p class="hint">모듈에 메인 클래스가 여러 개일 때 지정합니다.</p>
          </div>
        {/if}

        {#if usesArgs(kind)}
          <label for="f-args">{argsHelp.label}</label>
          <div>
            <input id="f-args" value={joinArgs(draft.args)} onchange={(e) => (draft.args = splitArgs(e.currentTarget.value))} placeholder={argsHelp.placeholder} spellcheck="false" class="mono" />
            {#if argsHelp.hint}<p class="hint">{argsHelp.hint}</p>{/if}
          </div>
        {/if}
      </div>
    </section>

    {#if JAVA_KINDS.includes(kind) || NODE_KINDS.includes(kind)}
      <section>
        <h3>{NODE_KINDS.includes(kind) ? 'Node' : 'Java · Spring'}</h3>
        <div class="grid">
          {#if JAVA_KINDS.includes(kind) && draft.runtime}
            <label for="f-jdk">JDK</label>
            <div>
              <RuntimeSelect id="f-jdk" kind="jdk" value={draft.runtime.jdk} onchange={(v) => draft.runtime && (draft.runtime.jdk = v)} />
              <p class="hint">이 서비스에만 JAVA_HOME과 PATH를 바꿔 실행합니다.</p>
            </div>
          {/if}
          {#if MAVEN_KINDS.includes(kind) && draft.runtime}
            <label for="f-maven">Maven</label>
            <RuntimeSelect id="f-maven" kind="maven" value={draft.runtime.maven} onchange={(v) => draft.runtime && (draft.runtime.maven = v)} />
          {/if}
          {#if NODE_KINDS.includes(kind) && draft.runtime}
            <label for="f-node">Node</label>
            <div>
              <RuntimeSelect id="f-node" kind="node" value={draft.runtime.node} onchange={(v) => draft.runtime && (draft.runtime.node = v)} />
              <p class="hint">이 서비스에만 PATH 앞에 추가합니다.</p>
            </div>
          {/if}
          {#if SPRING_KINDS.includes(kind) && draft.spring}
            <label for="f-prof">Spring 프로파일</label>
            <input id="f-prof" value={(draft.spring.profiles ?? []).join(', ')} onchange={(e) => draft.spring && (draft.spring.profiles = splitList(e.currentTarget.value))} placeholder="local, db" spellcheck="false" class="mono" />
          {/if}
          {#if usesJvmArgs(kind)}
            <label for="f-jvm">VM 옵션</label>
            <div>
              <textarea id="f-jvm" rows="2" value={joinArgs(draft.jvmArgs)} onchange={(e) => (draft.jvmArgs = splitArgs(e.currentTarget.value))} placeholder="-Xmx1g -Dfile.encoding=UTF-8" spellcheck="false" class="mono"></textarea>
              {#if kind === 'gradle-boot' || kind === 'gradle'}
                <p class="hint">Gradle init script로 주입합니다. build.gradle은 수정하지 않습니다.</p>
              {/if}
            </div>
          {/if}
        </div>
      </section>
    {/if}

    <section>
      <h3>환경</h3>
      <div class="grid">
        <span class="label">환경변수</span>
        <KeyValueEditor value={draft.env} resetKey={loadedId + ':' + formRev} onchange={(v: EnvMap) => (draft.env = v)} keyPlaceholder="NAME" addLabel="환경변수 추가" />

        <label for="f-envfiles">env 파일</label>
        <div>
          <textarea id="f-envfiles" rows="2" value={(draft.envFiles ?? []).join('\n')} onchange={(e) => (draft.envFiles = splitLines(e.currentTarget.value))} placeholder=".env.local (한 줄에 하나, 작업 디렉터리 기준)" spellcheck="false" class="mono"></textarea>
          <p class="hint">적용 순서: 워크스페이스 → 그룹 → env 파일 → 이 서비스 (뒤가 우선)</p>
        </div>

        {#if usesUtf8Inject(kind) || usesColorInject(kind)}
          <span class="label">자동 옵션</span>
          <div class="checks">
            {#if usesUtf8Inject(kind)}
              <label class="check">
                <input type="checkbox" checked={draft.inject?.utf8 !== false} onchange={(e) => setInject('utf8', e.currentTarget.checked)} />
                한글 깨짐 방지 (-Dfile.encoding=UTF-8, -Dstdout.encoding=UTF-8 등)
              </label>
            {/if}
            {#if usesColorInject(kind)}
              <label class="check">
                <input type="checkbox" checked={draft.inject?.color !== false} onchange={(e) => setInject('color', e.currentTarget.checked)} />
                컬러 로그 강제 ({kind === 'npm' || kind === 'node' ? 'FORCE_COLOR=1' : kind === 'maven' ? '-Dstyle.color=always' : '--spring.output.ansi.enabled=always'})
              </label>
            {/if}
            <p class="hint">이미 같은 옵션을 직접 지정했다면 그 값을 그대로 씁니다.</p>
          </div>
        {/if}

        <label for="f-enc">출력 인코딩</label>
        <div>
          <select id="f-enc" bind:value={draft.encoding}>
            <option value="auto">자동 (UTF-8, 실패 시 CP949)</option>
            <option value="utf-8">UTF-8</option>
            <option value="cp949">CP949 (EUC-KR)</option>
          </select>
        </div>

        <label for="f-ports">포트</label>
        <div>
          <input id="f-ports" value={(draft.ports ?? []).join(', ')} onchange={(e) => (draft.ports = parsePorts(e.currentTarget.value))} placeholder="8080" spellcheck="false" class="mono" />
          <p class="hint">시작 전에 이 포트(와 지난번 실행에서 감지한 포트)를 다른 프로세스가 쓰는지 확인합니다. 포트가 열리면 준비 완료로 봅니다.</p>
        </div>

        {#if draft.readiness}
          <span class="label">준비 감지</span>
          <div class="checks">
            <label class="check">
              <input type="checkbox" checked={draft.readiness.disabled !== true} onchange={(e) => draft.readiness && (draft.readiness.disabled = !e.currentTarget.checked)} />
              준비될 때까지 '시작 중'으로 표시
            </label>
            {#if draft.readiness.disabled !== true}
              <input
                aria-label="준비 로그 패턴"
                bind:value={() => draft.readiness?.logPattern ?? '', (v) => draft.readiness && (draft.readiness.logPattern = v)}
                placeholder={DEFAULT_READY_HINT[kind] ? `기본: ${DEFAULT_READY_HINT[kind]}` : '로그 정규식 (예: Server started)'}
                spellcheck="false"
                class="mono"
              />
              <input
                aria-label="HTTP 헬스 URL"
                bind:value={() => draft.readiness?.httpUrl ?? '', (v) => draft.readiness && (draft.readiness.httpUrl = v)}
                placeholder="HTTP 헬스 URL (예: http://localhost:8080/actuator/health)"
                spellcheck="false"
                class="mono"
              />
              <div class="inline">
                <input
                  aria-label="제한 시간"
                  type="number"
                  min="1"
                  max="3600"
                  class="num"
                  value={draft.readiness.timeoutSecs ?? ''}
                  placeholder={['gradle-boot', 'maven-boot', 'java-main'].includes(kind) ? '300' : '120'}
                  onchange={(e) => draft.readiness && (draft.readiness.timeoutSecs = e.currentTarget.value ? Number(e.currentTarget.value) : undefined)}
                />
                <span class="hint">초 안에 신호가 없으면 '실행 중'으로 바꿉니다.</span>
              </div>
              <p class="hint">로그 패턴·포트 열림·HTTP 응답 중 하나라도 확인되면 준비 완료입니다. 패턴을 비우면 타입별 기본값을 씁니다.</p>
            {/if}
          </div>
        {/if}

        <label for="f-notes">메모</label>
        <textarea id="f-notes" rows="2" bind:value={() => draft.notes ?? '', (v) => (draft.notes = v)}></textarea>
      </div>
    </section>

    <section class="preview">
      <h3>실행 명령 미리보기</h3>
      {#if preview?.error}
        <p class="preview-err">{preview.error}</p>
      {:else if preview}
        <code class="cmd">{preview.command}</code>
        <p class="hint">작업 디렉터리: <span class="mono">{preview.cwd}</span></p>
        {#each previewEnv as [k, v] (k)}
          <p class="hint">{k}: <span class="mono">{v}</span></p>
        {/each}
      {:else}
        <p class="hint">계산 중…</p>
      {/if}
    </section>
  </div>

  <footer class="actions">
    <button type="button" class="danger-link" onclick={() => app.deleteService(service.id)}><Icon name="trash" size={14} /> 삭제</button>
    <button type="button" class="link" onclick={() => app.duplicateService(service.id)}><Icon name="copy" size={14} /> 복제</button>
    <span class="spacer"></span>
    {#if dirty}<span class="dirty">저장하지 않은 변경 사항</span>{/if}
    <button type="button" disabled={!dirty || saving} onclick={revert}>되돌리기</button>
    {#if running}
      <button type="button" disabled={!dirty || saving} onclick={() => save(true)}>저장 후 재시작</button>
    {/if}
    <button type="submit" class="primary" disabled={!dirty || saving}>저장 <kbd>Ctrl+S</kbd></button>
  </footer>
</form>

<style>
  .form {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 8px 20px 24px;
  }
  section {
    max-width: 860px;
    padding: 12px 0 4px;
    border-bottom: 1px solid var(--border);
  }
  section:last-child {
    border-bottom: 0;
  }
  h3 {
    margin: 0 0 10px;
    color: var(--muted);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
  }
  .grid {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr);
    gap: 8px 14px;
    align-items: start;
    padding-bottom: 8px;
  }
  .grid > label,
  .grid > .label {
    padding-top: 5px;
    color: var(--text-2);
  }
  input,
  select,
  textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 4px 8px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
    outline: 0;
  }
  input,
  select {
    height: 28px;
  }
  textarea {
    resize: vertical;
    line-height: 1.5;
  }
  input:focus,
  select:focus,
  textarea:focus {
    border-color: var(--accent);
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .with-btn {
    display: flex;
    gap: 4px;
  }
  .btn-icon {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--panel);
    color: var(--muted);
    cursor: pointer;
  }
  .btn-icon:hover {
    color: var(--text);
    border-color: var(--muted);
  }
  .source {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 860px;
    margin-top: 12px;
    padding: 8px 10px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--text-2);
    font-size: 12.5px;
  }
  .source .spacer {
    flex: 1;
  }
  .muted {
    color: var(--faint);
  }
  .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: none;
    padding: 3px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 5px;
    background: var(--panel);
    color: var(--text);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .link-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .checks {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 4px;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .inline .hint {
    margin: 0;
  }
  .num {
    width: 90px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .check input {
    width: auto;
    height: auto;
    accent-color: var(--accent);
  }
  .hint {
    margin: 4px 0 0;
    color: var(--faint);
    font-size: 11.5px;
  }
  .notice {
    margin: 0;
    padding: 8px 10px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--warn) 14%, transparent);
    color: var(--warn);
  }
  .preview .cmd {
    display: block;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--console-bg);
    color: var(--console-fg);
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
  .preview-err {
    margin: 0;
    padding: 8px 10px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--err) 12%, transparent);
    color: var(--err);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 20px;
    border-top: 1px solid var(--border);
    background: var(--panel);
  }
  .spacer {
    flex: 1;
  }
  .dirty {
    color: var(--warn);
    font-size: 12px;
  }
  .actions button {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .actions button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .actions button.primary:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
  .actions .link,
  .actions .danger-link {
    border-color: transparent;
    background: none;
    color: var(--muted);
    padding: 0 8px;
  }
  .actions .danger-link:hover {
    color: var(--err);
  }
  .actions .link:hover {
    color: var(--text);
  }
  kbd {
    font-family: inherit;
    font-size: 10.5px;
    opacity: 0.75;
  }
</style>
