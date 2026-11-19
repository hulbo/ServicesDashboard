<script lang="ts">
  import { app } from '../state.svelte'
  import type { EnvMap } from '../types'
  import Icon from './Icon.svelte'
  import KeyValueEditor from './KeyValueEditor.svelte'

  let name = $state(app.workspace.name)
  let variables = $state<EnvMap>({ ...(app.workspace.variables ?? {}) })
  let env = $state<EnvMap>({ ...(app.workspace.env ?? {}) })
  let rev = $state(0)

  const dirty = $derived(
    name !== app.workspace.name ||
      JSON.stringify(variables) !== JSON.stringify(app.workspace.variables ?? {}) ||
      JSON.stringify(env) !== JSON.stringify(app.workspace.env ?? {}),
  )

  function revert() {
    name = app.workspace.name
    variables = { ...(app.workspace.variables ?? {}) }
    env = { ...(app.workspace.env ?? {}) }
    rev++
  }

  async function save() {
    const ok = await app.mutate((ws) => {
      ws.name = name.trim() || ws.name
      if (Object.keys(variables).length) ws.variables = { ...variables }
      else delete ws.variables
      if (Object.keys(env).length) ws.env = { ...env }
      else delete ws.env
    })
    if (ok) app.toast('success', '워크스페이스 설정을 저장했습니다.')
  }
</script>

<div class="panel">
  <header class="head">
    <Icon name="workspace" size={18} />
    <h2>워크스페이스 설정</h2>
    <span class="spacer"></span>
    <button class="btn" disabled={!dirty} onclick={revert}>되돌리기</button>
    <button class="btn primary" disabled={!dirty} onclick={save}>저장</button>
  </header>

  <div class="scroll">
    <section>
      <h3>이름</h3>
      <input class="text" bind:value={name} spellcheck="false" />
    </section>

    <section>
      <h3>변수</h3>
      <p class="hint">
        서비스 설정에서 <code>${'{'}var:이름{'}'}</code>으로 참조합니다. 예: <code>ROOT = C:/Project/my-project</code> →
        작업 디렉터리 <code>${'{'}var:ROOT{'}'}/front/portal</code>. 그 밖에 <code>${'{'}env:USERPROFILE{'}'}</code>,
        <code>${'{'}userHome{'}'}</code>도 쓸 수 있습니다.
      </p>
      <KeyValueEditor value={variables} resetKey={'vars:' + rev} onchange={(v) => (variables = v)} keyPlaceholder="ROOT" valuePlaceholder="C:/Project/my-project" addLabel="변수 추가" />
    </section>

    <section>
      <h3>공통 환경변수</h3>
      <p class="hint">모든 서비스에 적용됩니다. 그룹·서비스에 같은 이름이 있으면 그쪽이 우선합니다.</p>
      <KeyValueEditor value={env} resetKey={'env:' + rev} onchange={(v) => (env = v)} keyPlaceholder="NAME" addLabel="환경변수 추가" />
    </section>

    <section>
      <h3>파일</h3>
      <p class="hint">설정은 아래 파일에 JSON으로 저장됩니다. 직접 고친 뒤에는 '다시 불러오기'를 누르세요.</p>
      <code class="path">{app.workspacePath}</code>
      <div class="row">
        {#if app.info}
          <button class="btn" onclick={() => app.info && app.backend.openPath(app.info.configDir)}><Icon name="folder" size={14} /> 설정 폴더 열기</button>
        {/if}
        <button class="btn" onclick={async () => { await app.loadWorkspace(true); revert() }}><Icon name="refresh" size={14} /> 다시 불러오기</button>
      </div>
    </section>
  </div>
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    color: var(--muted);
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .spacer {
    flex: 1;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 4px 20px 24px;
  }
  section {
    max-width: 760px;
    padding: 14px 0;
    border-bottom: 1px solid var(--border);
  }
  section:last-child {
    border-bottom: 0;
  }
  h3 {
    margin: 0 0 8px;
    color: var(--muted);
    font-size: 11.5px;
    font-weight: 600;
  }
  .hint {
    margin: 0 0 10px;
    color: var(--faint);
    font-size: 12px;
    line-height: 1.6;
  }
  code {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-2);
  }
  .text {
    width: 320px;
    max-width: 100%;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
    outline: 0;
  }
  .text:focus {
    border-color: var(--accent);
  }
  .path {
    display: block;
    padding: 8px 10px;
    margin-bottom: 10px;
    border-radius: 6px;
    background: var(--console-bg);
    word-break: break-all;
    user-select: text;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .btn {
    display: inline-flex;
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
  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn.primary:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
</style>
