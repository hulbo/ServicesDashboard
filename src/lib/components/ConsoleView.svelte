<script lang="ts">
  import { tick } from 'svelte'
  import { app } from '../state.svelte'
  import { errorMessage } from '../api'
  import Icon from './Icon.svelte'

  let { serviceId }: { serviceId: string } = $props()

  const ROW = 18
  const OVERSCAN = 30

  let viewport: HTMLDivElement | undefined = $state()
  let scrollTop = $state(0)
  let height = $state(400)
  let follow = $state(true)
  let seenSeq = $state(0)
  let lastRemoved = 0
  let lastId = ''

  const store = $derived(app.logStore(serviceId))
  const version = $derived(app.logTick[serviceId] ?? 0)
  const total = $derived.by(() => {
    void version
    return store.lines.length
  })
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN))
  const last = $derived(Math.min(total, Math.ceil((scrollTop + height) / ROW) + OVERSCAN))
  const visible = $derived.by(() => {
    void version
    return store.lines.slice(first, last)
  })
  const unseen = $derived.by(() => {
    void version
    return follow ? 0 : Math.max(0, store.lastSeq - seenSeq)
  })

  // 서비스를 바꾸면 맨 아래부터 보여 준다.
  $effect.pre(() => {
    if (serviceId !== lastId) {
      lastId = serviceId
      follow = true
      lastRemoved = store.removedTotal
      void app.ensureLogs(serviceId)
    }
  })

  // 새 로그가 오면: 따라가는 중이면 맨 아래로, 아니면 잘려 나간 줄만큼 위치를 보정한다.
  $effect(() => {
    void version
    const el = viewport
    if (!el) return
    const removed = store.removedTotal - lastRemoved
    lastRemoved = store.removedTotal
    if (follow) {
      seenSeq = store.lastSeq
      void tick().then(() => {
        el.scrollTop = el.scrollHeight
        scrollTop = el.scrollTop
      })
    } else if (removed > 0) {
      el.scrollTop = Math.max(0, el.scrollTop - removed * ROW)
      scrollTop = el.scrollTop
    }
  })

  function onScroll() {
    const el = viewport
    if (!el) return
    scrollTop = el.scrollTop
    const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - ROW * 1.5
    if (atBottom !== follow) {
      follow = atBottom
      if (atBottom) seenSeq = store.lastSeq
    }
  }

  function toBottom() {
    follow = true
    seenSeq = store.lastSeq
    if (viewport) viewport.scrollTop = viewport.scrollHeight
  }

  async function copyAll() {
    try {
      await navigator.clipboard.writeText(store.text())
      app.toast('success', `${store.lines.length.toLocaleString()}줄을 클립보드에 복사했습니다.`)
    } catch (e) {
      app.toast('error', `복사 실패: ${errorMessage(e)}`)
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.ctrlKey && (e.key === 'l' || e.key === 'L')) {
      e.preventDefault()
      void app.clearLogs(serviceId)
    } else if (e.key === 'End' && e.ctrlKey) {
      e.preventDefault()
      toBottom()
    }
  }
</script>

<div class="console">
  <div class="bar">
    <button class="tool" class:on={follow} title="자동 스크롤 (맨 아래 따라가기)" onclick={() => (follow ? (follow = false) : toBottom())}>
      <Icon name="arrow-down" size={14} />
      <span>자동 스크롤</span>
    </button>
    <span class="count">{total.toLocaleString()}줄</span>
    <span class="spacer"></span>
    <button class="tool" title="전체 복사" disabled={total === 0} onclick={copyAll}><Icon name="copy" size={14} /></button>
    <button class="tool" title="콘솔 지우기 (Ctrl+L)" disabled={total === 0} onclick={() => app.clearLogs(serviceId)}><Icon name="clear" size={14} /></button>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div class="viewport" bind:this={viewport} bind:clientHeight={height} onscroll={onScroll} tabindex="0" role="log" aria-live="off" onkeydown={onKeydown}>
    {#if total === 0}
      <div class="placeholder">
        <p>아직 로그가 없습니다.</p>
        <p class="sub">▶ 시작을 누르면 이곳에 콘솔 출력이 표시됩니다.</p>
      </div>
    {:else}
      <div class="spacer-box" style="height:{total * ROW}px">
        <div class="rows" style="transform:translateY({first * ROW}px)">
          {#each visible as line (line.seq)}
            <div class="line s{line.stream}">{#each line.segments as seg, i (i)}<span style={seg.style}>{seg.text}</span>{/each}{#if line.segments.length === 0}&#8203;{/if}</div>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  {#if unseen > 0}
    <button class="new-lines" onclick={toBottom}><Icon name="arrow-down" size={13} /> 새 로그 {unseen.toLocaleString()}줄</button>
  {/if}
</div>

<style>
  .console {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--console-bg);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 32px;
    padding: 0 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel-2);
  }
  .tool {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 7px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .tool:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .tool.on {
    color: var(--accent-text);
  }
  .tool:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .count {
    color: var(--faint);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
    margin-left: 4px;
  }
  .spacer {
    flex: 1;
  }
  .viewport {
    flex: 1;
    min-height: 0;
    overflow: auto;
    outline: none;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 18px;
    color: var(--console-fg);
    tab-size: 4;
  }
  .spacer-box {
    position: relative;
    width: max-content;
    min-width: 100%;
  }
  .rows {
    padding: 0 12px;
    will-change: transform;
  }
  .line {
    height: 18px;
    white-space: pre;
  }
  .line.s1 {
    color: var(--console-err);
  }
  .line.s2 {
    color: var(--console-sys);
  }
  .line ::selection,
  .line::selection {
    background: var(--console-selection);
  }
  .placeholder {
    padding: 40px 24px;
    font-family: var(--font-ui);
    color: var(--muted);
    text-align: center;
  }
  .placeholder p {
    margin: 4px 0;
  }
  .placeholder .sub {
    color: var(--faint);
    font-size: 12px;
  }
  .new-lines {
    position: absolute;
    right: 20px;
    bottom: 16px;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px 12px;
    border: 0;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font: inherit;
    font-size: 12px;
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.35);
    cursor: pointer;
  }
</style>
