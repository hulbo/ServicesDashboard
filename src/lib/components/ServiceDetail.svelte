<script lang="ts">
  import { app } from '../state.svelte'
  import type { Service } from '../types'
  import { KIND_LABELS, STATE_LABELS, isActive } from '../types'
  import ConsoleView from './ConsoleView.svelte'
  import Icon from './Icon.svelte'
  import ServiceForm from './ServiceForm.svelte'
  import StatusDot from './StatusDot.svelte'

  let { service }: { service: Service } = $props()

  const st = $derived(app.statuses[service.id])
  const state = $derived(st?.state ?? 'stopped')
  const active = $derived(isActive(state))
  const groupName = $derived(app.workspace.groups.find((g) => g.id === service.group)?.name)

  function elapsed(from: number | null | undefined, to: number): string {
    if (!from) return ''
    const s = Math.max(0, Math.floor((to - from) / 1000))
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const sec = s % 60
    if (h) return `${h}시간 ${m}분`
    if (m) return `${m}분 ${sec}초`
    return `${sec}초`
  }

  const detail = $derived.by(() => {
    if (!st) return ''
    if (state === 'running' || state === 'stopping') return `PID ${st.pid ?? '-'} · ${elapsed(st.startedAt, app.now)}`
    if (state === 'failed' && st.exitCode !== null) return `종료 코드 ${st.exitCode}`
    if (state === 'failed' && st.error) return st.error
    return ''
  })
</script>

<div class="detail">
  <header class="head">
    <div class="title">
      <StatusDot {state} size={10} />
      <h2 title={service.name}>{service.name}</h2>
      <span class="state st-{state}">{STATE_LABELS[state]}</span>
      {#if detail}<span class="sub" title={detail}>{detail}</span>{/if}
      {#if service.ports?.length}
        <span class="ports">
          {#each service.ports as p (p)}
            <button class="port" class:live={active} title="브라우저로 열기" onclick={() => app.backend.openUrl(`http://localhost:${p}`)}>
              :{p} <Icon name="external" size={11} />
            </button>
          {/each}
        </span>
      {/if}
    </div>
    <div class="buttons">
      {#if active}
        <button class="btn stop" title="중지 (Ctrl+F2)" disabled={state === 'stopping'} onclick={() => app.stop(service.id)}><Icon name="stop" size={14} /> 중지</button>
        <button class="btn" title="재시작 (Ctrl+F5)" disabled={state === 'stopping'} onclick={() => app.restart(service.id)}><Icon name="restart" size={14} /> 재시작</button>
      {:else}
        <button class="btn run" title="시작 (Shift+F10)" onclick={() => app.start(service.id)}><Icon name="play" size={14} /> 시작</button>
      {/if}
    </div>
  </header>

  <nav class="tabs">
    <button class:active={app.tab === 'console'} onclick={() => (app.tab = 'console')}><Icon name="terminal" size={14} /> 콘솔</button>
    <button class:active={app.tab === 'settings'} onclick={() => (app.tab = 'settings')}><Icon name="gear" size={14} /> 설정</button>
    <span class="meta">{KIND_LABELS[service.type]}{groupName ? ` · ${groupName}` : ''}</span>
  </nav>

  <div class="body">
    {#if app.tab === 'console'}
      <ConsoleView serviceId={service.id} />
    {:else}
      <ServiceForm {service} />
    {/if}
  </div>
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    background: var(--panel);
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .state {
    flex: none;
    font-size: 12px;
    color: var(--muted);
  }
  .state.st-running {
    color: var(--ok);
  }
  .state.st-failed {
    color: var(--err);
  }
  .state.st-starting,
  .state.st-stopping {
    color: var(--warn);
  }
  .sub {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--faint);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .ports {
    display: flex;
    gap: 4px;
  }
  .port {
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 1px 6px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    background: none;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11.5px;
    cursor: pointer;
  }
  .port.live {
    color: var(--ok);
    border-color: color-mix(in srgb, var(--ok) 45%, transparent);
  }
  .port:hover {
    color: var(--text);
  }
  .buttons {
    display: flex;
    gap: 6px;
  }
  .btn {
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
  .btn:hover:not(:disabled) {
    background: var(--hover);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .btn.run {
    border-color: transparent;
    background: var(--ok-strong);
    color: #fff;
  }
  .btn.run:hover {
    background: var(--ok-strong);
    filter: brightness(1.1);
  }
  .btn.stop {
    color: var(--err);
  }
  .tabs {
    display: flex;
    align-items: flex-end;
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
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .tabs .meta {
    margin-left: auto;
    padding-bottom: 8px;
    color: var(--faint);
    font-size: 12px;
  }
  .body {
    flex: 1;
    min-height: 0;
  }
</style>
