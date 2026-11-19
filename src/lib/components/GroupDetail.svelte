<script lang="ts">
  import { untrack } from 'svelte'
  import { app } from '../state.svelte'
  import type { EnvMap, Group } from '../types'
  import { KIND_LABELS, STATE_LABELS, isActive } from '../types'
  import Icon from './Icon.svelte'
  import KeyValueEditor from './KeyValueEditor.svelte'
  import StatusDot from './StatusDot.svelte'
  import MergedConsole from './MergedConsole.svelte'

  let { group }: { group: Group } = $props()

  const services = $derived(app.workspace.services.filter((s) => s.group === group.id))
  let tab = $state<'info' | 'logs'>('info')
  const running = $derived(services.filter((s) => isActive(app.statuses[s.id]?.state)).length)

  let envDraft = $state<EnvMap>({})
  let loaded = ''
  let rev = $state(0)
  $effect.pre(() => {
    if (group.id !== loaded) {
      loaded = group.id
      envDraft = { ...(group.env ?? {}) }
      untrack(() => rev++)
    }
  })
  const envDirty = $derived(JSON.stringify(envDraft) !== JSON.stringify(group.env ?? {}))

  async function saveEnv() {
    const g = { ...($state.snapshot(group) as Group) }
    if (Object.keys(envDraft).length) g.env = { ...envDraft }
    else delete g.env
    if (await app.updateGroup(g)) app.toast('success', '그룹 환경변수를 저장했습니다.')
  }
</script>

<div class="panel">
  <header class="head">
    <span class="folder"><Icon name="folder" size={18} /></span>
    {#if app.renaming === group.id}
      <!-- svelte-ignore a11y_autofocus -->
      <input class="rename" value={group.name} autofocus onkeydown={(e) => { if (e.key === 'Enter') app.rename(group.id, e.currentTarget.value); if (e.key === 'Escape') app.renaming = null }} onblur={(e) => app.rename(group.id, e.currentTarget.value)} />
    {:else}
      <h2>{group.name}</h2>
      <button class="link" title="이름 바꾸기 (F2)" onclick={() => (app.renaming = group.id)}><Icon name="edit" size={13} /></button>
    {/if}
    <span class="count">실행 {running} / {services.length}</span>
    <span class="spacer"></span>
    <button class="btn" disabled={running === 0} onclick={() => app.stopGroup(group.id)}><Icon name="stop" size={14} /> 전체 중지</button>
    <button class="btn run" disabled={services.length === 0 || running === services.length} onclick={() => app.startGroup(group.id)}><Icon name="play-all" size={14} /> 전체 시작</button>
  </header>

  <nav class="tabs">
    <button class:active={tab === 'info'} onclick={() => (tab = 'info')}><Icon name="gear" size={14} /> 서비스·설정</button>
    <button class:active={tab === 'logs'} onclick={() => (tab = 'logs')}><Icon name="terminal" size={14} /> 통합 로그</button>
  </nav>
  {#if tab === 'logs'}
    <div class="body"><MergedConsole serviceIds={services.map((s) => s.id)} /></div>
  {:else}
  <div class="scroll">
    <section>
      <h3>서비스</h3>
      {#if services.length === 0}
        <p class="empty">이 그룹에 서비스가 없습니다. 트리에서 서비스를 끌어다 놓거나 새로 추가하세요.</p>
      {:else}
        <ul class="services">
          {#each services as s (s.id)}
            {@const st = app.statuses[s.id]?.state ?? 'stopped'}
            <li>
              <StatusDot state={st} />
              <button class="name" onclick={() => app.select({ kind: 'service', id: s.id })}>{s.name}</button>
              <span class="kind">{KIND_LABELS[s.type]}</span>
              <span class="st">{STATE_LABELS[st]}</span>
              {#if isActive(st)}
                <button class="mini stop" title="중지" onclick={() => app.stop(s.id)}><Icon name="stop" size={13} /></button>
              {:else}
                <button class="mini play" title="시작" onclick={() => app.start(s.id)}><Icon name="play" size={13} /></button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
      <button class="add" onclick={() => app.addService(group.id)}><Icon name="plus" size={13} /> 이 그룹에 서비스 추가</button>
    </section>

    <section>
      <h3>그룹 환경변수</h3>
      <p class="hint">이 그룹의 모든 서비스에 적용됩니다. 서비스에 같은 이름이 있으면 서비스 값이 우선합니다.</p>
      <KeyValueEditor value={envDraft} resetKey={group.id + ':' + rev} onchange={(v) => (envDraft = v)} keyPlaceholder="NAME" addLabel="환경변수 추가" />
      <div class="save-row">
        <button class="btn primary" disabled={!envDirty} onclick={saveEnv}>저장</button>
      </div>
    </section>

    <section>
      <h3>관리</h3>
      <button class="btn danger" onclick={() => app.deleteGroup(group.id)}><Icon name="trash" size={14} /> 그룹 삭제</button>
      <p class="hint">그룹 안의 서비스는 지워지지 않고 '그룹 없음'으로 옮겨집니다.</p>
    </section>
  </div>
  {/if}
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
  }
  .folder {
    color: var(--folder);
    display: flex;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px 7px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    font: inherit;
    cursor: pointer;
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .body {
    flex: 1;
    min-height: 0;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .rename {
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--accent);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
    font-size: 14px;
    outline: 0;
  }
  .count {
    color: var(--muted);
    font-size: 12px;
  }
  .spacer {
    flex: 1;
  }
  .link {
    display: grid;
    place-items: center;
    padding: 3px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
    background: var(--hover);
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
    margin: 0 0 10px;
    color: var(--muted);
    font-size: 11.5px;
    font-weight: 600;
  }
  .hint,
  .empty {
    margin: 0 0 10px;
    color: var(--faint);
    font-size: 12px;
  }
  .services {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  .services li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
  }
  .services li:last-child {
    border-bottom: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name:hover {
    color: var(--accent-text);
    text-decoration: underline;
  }
  .kind,
  .st {
    color: var(--faint);
    font-size: 12px;
  }
  .st {
    width: 52px;
    text-align: right;
  }
  .mini {
    display: grid;
    place-items: center;
    width: 24px;
    height: 22px;
    border: 0;
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }
  .mini:hover {
    background: var(--hover);
  }
  .mini.play {
    color: var(--ok);
  }
  .mini.stop {
    color: var(--err);
  }
  .add {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border: 1px dashed var(--border-strong);
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .add:hover {
    color: var(--text);
  }
  .save-row {
    margin-top: 10px;
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
  .btn.run:not(:disabled) {
    border-color: transparent;
    background: var(--ok-strong);
    color: #fff;
  }
  .btn.primary:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
  .btn.danger {
    color: var(--err);
    margin-bottom: 8px;
  }
</style>
