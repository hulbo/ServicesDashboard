<script lang="ts">
  import { untrack } from 'svelte'
  import { app } from '../state.svelte'
  import type { Preset } from '../types'
  import { STATE_LABELS, isActive } from '../types'
  import Icon from './Icon.svelte'
  import MergedConsole from './MergedConsole.svelte'
  import StatusDot from './StatusDot.svelte'

  let { preset }: { preset: Preset } = $props()

  let tab = $state<'services' | 'logs'>('services')
  let picked = $state<string[]>(untrack(() => [...preset.services]))
  let loaded = untrack(() => preset.id)
  $effect.pre(() => {
    if (preset.id !== loaded) {
      loaded = preset.id
      picked = [...preset.services]
    }
  })

  const members = $derived(preset.services.filter((id) => app.workspace.services.some((s) => s.id === id)))
  const running = $derived(members.filter((id) => isActive(app.statuses[id]?.state)).length)
  const dirty = $derived(JSON.stringify([...picked].sort()) !== JSON.stringify([...preset.services].sort()))
  /** 그룹별로 묶은 서비스 목록 (체크리스트용) */
  const sections = $derived([
    ...app.workspace.groups.map((g) => ({ name: g.name, services: app.workspace.services.filter((s) => s.group === g.id) })),
    { name: '그룹 없음', services: app.workspace.services.filter((s) => !s.group) },
  ].filter((x) => x.services.length))
  /** 고른 서비스가 의존하는 서비스 (함께 시작된다) */
  const implied = $derived.by(() => {
    const out = new Set<string>()
    const visit = (id: string) => {
      for (const d of app.workspace.services.find((s) => s.id === id)?.dependsOn ?? []) {
        if (!out.has(d)) {
          out.add(d)
          visit(d)
        }
      }
    }
    picked.forEach(visit)
    return out
  })

  function toggle(id: string) {
    picked = picked.includes(id) ? picked.filter((x) => x !== id) : [...picked, id]
  }

  async function save() {
    // 워크스페이스 서비스 순서대로 저장
    const order = app.workspace.services.map((s) => s.id).filter((id) => picked.includes(id))
    if (await app.updatePreset({ ...($state.snapshot(preset) as Preset), services: order })) app.toast('success', '프리셋을 저장했습니다.')
  }
</script>

<div class="panel">
  <header class="head">
    <Icon name="play-all" size={16} />
    {#if app.renaming === preset.id}
      <!-- svelte-ignore a11y_autofocus -->
      <input class="rename" value={preset.name} autofocus onkeydown={(e) => { if (e.key === 'Enter') app.rename(preset.id, e.currentTarget.value); if (e.key === 'Escape') app.renaming = null }} onblur={(e) => app.rename(preset.id, e.currentTarget.value)} />
    {:else}
      <h2>{preset.name}</h2>
      <button class="link" title="이름 바꾸기" onclick={() => (app.renaming = preset.id)}><Icon name="edit" size={13} /></button>
    {/if}
    <span class="count">실행 {running} / {members.length}</span>
    <span class="spacer"></span>
    <button class="btn" disabled={running === 0} onclick={() => app.stopPreset(preset.id)}><Icon name="stop" size={14} /> 전체 중지</button>
    <button class="btn run" disabled={members.length === 0} onclick={() => app.startPreset(preset.id)}><Icon name="play-all" size={14} /> 전체 시작</button>
  </header>
  <nav class="tabs">
    <button class:active={tab === 'services'} onclick={() => (tab = 'services')}><Icon name="gear" size={14} /> 서비스</button>
    <button class:active={tab === 'logs'} onclick={() => (tab = 'logs')}><Icon name="terminal" size={14} /> 통합 로그</button>
  </nav>

  {#if tab === 'logs'}
    <div class="body"><MergedConsole serviceIds={members} /></div>
  {:else}
    <div class="scroll">
      <p class="hint">그룹과 상관없이 한 번에 실행할 서비스를 고릅니다. 의존 관계가 있으면 의존 대상이 먼저 준비된 뒤 차례로 시작합니다.</p>
      {#each sections as sec (sec.name)}
        <h3>{sec.name}</h3>
        <ul class="list">
          {#each sec.services as s (s.id)}
            {@const st = app.statuses[s.id]?.state ?? 'stopped'}
            <li>
              <label class="check">
                <input type="checkbox" checked={picked.includes(s.id)} onchange={() => toggle(s.id)} />
                <StatusDot state={st} />
                <span class="name">{s.name}</span>
              </label>
              {#if !picked.includes(s.id) && implied.has(s.id)}<span class="implied">의존 대상으로 함께 시작</span>{/if}
              <span class="st">{STATE_LABELS[st]}</span>
            </li>
          {/each}
        </ul>
      {/each}
      <div class="actions">
        <button class="btn danger" onclick={() => app.deletePreset(preset.id)}><Icon name="trash" size={14} /> 프리셋 삭제</button>
        <span class="spacer"></span>
        <button class="btn" disabled={!dirty} onclick={() => (picked = [...preset.services])}>되돌리기</button>
        <button class="btn primary" disabled={!dirty} onclick={save}>저장</button>
      </div>
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
    color: var(--accent-text);
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .rename {
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--accent);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
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
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 8px 20px 24px;
  }
  .hint {
    max-width: 760px;
    color: var(--faint);
    font-size: 12px;
  }
  h3 {
    margin: 14px 0 6px;
    color: var(--muted);
    font-size: 11.5px;
    font-weight: 600;
  }
  .list {
    max-width: 760px;
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
  }
  .list li:last-child {
    border-bottom: 0;
  }
  .check {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .check input {
    accent-color: var(--accent);
  }
  .implied {
    color: var(--accent-text);
    font-size: 11.5px;
  }
  .st {
    color: var(--faint);
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 8px;
    max-width: 760px;
    margin-top: 16px;
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
  }
</style>
