<script lang="ts">
  import { untrack } from 'svelte'
  import { app } from '../state.svelte'
  import { errorMessage } from '../api'
  import { joinArgs, parsePorts, splitArgs, splitLines, splitList } from '../args'
  import { cleanForSave, normalizeForEdit, usesArgs, usesJvmArgs } from '../service-edit'
  import type { EnvMap, Service, ServiceKind } from '../types'
  import { JAVA_KINDS, KIND_LABELS, NODE_KINDS, SPRING_KINDS, isActive } from '../types'
  import Icon from './Icon.svelte'
  import KeyValueEditor from './KeyValueEditor.svelte'

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
  let preview = $state<{ command?: string; cwd?: string; error?: string } | null>(null)
  let previewSeq = 0
  $effect(() => {
    const s = cleanForSave(snapshot(draft))
    const seq = ++previewSeq
    const t = setTimeout(async () => {
      try {
        const p = await app.backend.preview(s)
        if (seq === previewSeq) preview = { command: p.command, cwd: p.cwd }
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
            <label for="f-jdk">JDK 경로</label>
            <div>
              <div class="with-btn">
                <input id="f-jdk" bind:value={() => draft.runtime?.jdk ?? '', (v) => draft.runtime && (draft.runtime.jdk = v)} placeholder="비우면 시스템 JAVA_HOME" spellcheck="false" class="mono" />
                <button type="button" class="btn-icon" title="폴더 선택" onclick={() => browseFolder(draft.runtime?.jdk, (p) => draft.runtime && (draft.runtime.jdk = p))}><Icon name="folder" size={14} /></button>
              </div>
              <p class="hint">이 서비스에만 JAVA_HOME과 PATH를 바꿔 실행합니다.</p>
            </div>
          {/if}
          {#if NODE_KINDS.includes(kind) && draft.runtime}
            <label for="f-node">Node 경로</label>
            <div>
              <div class="with-btn">
                <input id="f-node" bind:value={() => draft.runtime?.node ?? '', (v) => draft.runtime && (draft.runtime.node = v)} placeholder="비우면 PATH의 node" spellcheck="false" class="mono" />
                <button type="button" class="btn-icon" title="폴더 선택" onclick={() => browseFolder(draft.runtime?.node, (p) => draft.runtime && (draft.runtime.node = p))}><Icon name="folder" size={14} /></button>
              </div>
              <p class="hint">node.exe가 들어 있는 폴더. 이 서비스에만 PATH 앞에 추가합니다.</p>
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
          <p class="hint">트리에 표시합니다. 포트 충돌 검사는 M3에서 추가됩니다.</p>
        </div>

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
