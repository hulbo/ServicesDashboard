<script lang="ts">
  import { tick, untrack } from 'svelte'
  import { app } from '../state.svelte'
  import { compileQuery, decorate } from '../console-view'
  import type { ViewLine } from '../logstore'
  import Icon from './Icon.svelte'

  /** 여러 서비스의 로그를 시간순으로 합쳐 보여 준다 (그룹·프리셋의 '통합 로그') */
  let { serviceIds }: { serviceIds: string[] } = $props()

  /** 한 줄 높이: 설정의 콘솔 글자 크기를 따른다 (CSS --console-row와 같은 값) */
  const ROW = $derived(app.consoleRow)
  const OVERSCAN = 30
  /** 서비스마다 최근 이만큼만 합친다 (합치는 비용을 일정하게) */
  const PER_SERVICE = 4000
  /** 서비스별 색: 트리 순서로 정한 같은 색을 기둥 아이콘 테두리도 쓴다 */
  const colors = $derived(serviceIds.map((id) => app.colorOf(id)))

  type Row = { key: string; idx: number; line: ViewLine }

  let viewport: HTMLDivElement | undefined = $state()
  let scrollTop = $state(0)
  let height = $state(400)
  let follow = $state(true)
  let query = $state('')
  let hidden = $state<string[]>([])
  let rows = $state.raw<Row[]>([])
  let timer: ReturnType<typeof setTimeout> | null = null

  const names = $derived(serviceIds.map((id) => app.serviceName(id)))
  const nameWidth = $derived(Math.min(18, Math.max(4, ...names.map((n) => n.length))))
  const queryRe = $derived.by(() => {
    const q = compileQuery(query, false, false)
    return q instanceof RegExp ? q : null
  })
  const tickSum = $derived(serviceIds.reduce((a, id) => a + (app.logTick[id] ?? 0), 0))

  function rebuild() {
    timer = null
    const out: Row[] = []
    serviceIds.forEach((id, idx) => {
      if (hidden.includes(id)) return
      const lines = app.logStore(id).lines
      for (const line of lines.slice(-PER_SERVICE)) out.push({ key: `${id}:${line.seq}`, idx, line })
    })
    out.sort((a, b) => a.line.ts - b.line.ts || a.idx - b.idx || a.line.seq - b.line.seq)
    const re = queryRe
    rows = re
      ? out.filter((r) => {
          re.lastIndex = 0
          return re.test(r.line.plain)
        })
      : out
  }

  // 로그가 계속 들어와도 0.25초에 한 번만 다시 합친다
  $effect(() => {
    void tickSum
    void queryRe
    void hidden.length
    void serviceIds.join()
    untrack(() => {
      if (!timer) timer = setTimeout(rebuild, rows.length ? 250 : 0)
    })
    return () => {}
  })

  $effect(() => {
    for (const id of serviceIds) void app.ensureLogs(id)
  })

  $effect(() => {
    void rows
    const el = viewport
    if (el && untrack(() => follow)) {
      void tick().then(() => {
        el.scrollTop = el.scrollHeight
        scrollTop = el.scrollTop
      })
    }
  })

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN))
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + height) / ROW) + OVERSCAN))
  const visible = $derived(rows.slice(first, last))

  function onScroll() {
    const el = viewport
    if (!el) return
    scrollTop = el.scrollTop
    follow = el.scrollTop + el.clientHeight >= el.scrollHeight - ROW * 1.5
  }

  function toggle(id: string) {
    hidden = hidden.includes(id) ? hidden.filter((x) => x !== id) : [...hidden, id]
  }

  /** Ctrl+A: 통합 로그 내용 전체 선택 (보이는 줄만 그리므로 직접 표시·복사한다) */
  let allSelected = $state(false)

  function selectionText(): string {
    return rows.map((r) => `${names[r.idx].padEnd(nameWidth)} ${r.line.plain}`).join('\n')
  }

  async function copySelection() {
    try {
      await navigator.clipboard.writeText(selectionText())
      app.toast('success', `${rows.length.toLocaleString()}줄을 클립보드에 복사했습니다.`)
    } catch (e) {
      app.toast('error', `복사 실패: ${e instanceof Error ? e.message : String(e)}`)
    }
  }

  function onKeydown(e: KeyboardEvent) {
    const editing = (e.target as HTMLElement | null)?.closest('input, textarea, select')
    if (editing) return
    if (e.ctrlKey && !e.altKey && (e.key === 'a' || e.key === 'A')) {
      e.preventDefault()
      allSelected = true
      window.getSelection()?.removeAllRanges()
      viewport?.focus()
    } else if (allSelected && e.ctrlKey && (e.key === 'c' || e.key === 'C' || e.key === 'Insert')) {
      e.preventDefault()
      void copySelection()
    } else if (allSelected && e.key === 'Escape') {
      allSelected = false
    }
  }

  function onCopy(e: ClipboardEvent) {
    if (!allSelected || !e.clipboardData) return
    e.preventDefault()
    e.clipboardData.setData('text/plain', selectionText())
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="merged" onkeydown={onKeydown}>
  <div class="bar">
    <label class="search">
      <Icon name="search" size={13} />
      <input bind:value={query} placeholder="필터 (이 문자열이 있는 줄만)" spellcheck="false" aria-label="통합 로그 필터" />
    </label>
    <div class="chips">
      {#each serviceIds as id, i (id)}
        <button class="chip" class:off={hidden.includes(id)} style="--c:{colors[i]}" title="보이기/숨기기" onclick={() => toggle(id)}>
          <span class="dot"></span>{names[i]}
        </button>
      {/each}
    </div>
    <span class="spacer"></span>
    <span class="count">{rows.length.toLocaleString()}줄</span>
    <button class="tool" class:on={follow} title="자동 스크롤" onclick={() => { follow = true; if (viewport) viewport.scrollTop = viewport.scrollHeight }}><Icon name="arrow-down" size={14} /></button>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="viewport"
    class:all-selected={allSelected}
    bind:this={viewport}
    bind:clientHeight={height}
    onscroll={onScroll}
    onmousedown={(e) => e.button === 0 && (allSelected = false)}
    oncopy={onCopy}
    tabindex="0"
    role="log"
  >
    {#if rows.length === 0}
      <div class="placeholder">아직 로그가 없습니다. 서비스를 시작하면 여기에 시간순으로 모여 표시됩니다.</div>
    {:else}
      <div class="spacer-box" style="height:{rows.length * ROW}px">
        <div class="rows" style="transform:translateY({first * ROW}px)">
          {#each visible as r (r.key)}
            <div class="line s{r.line.stream} lv{r.line.level}"><span class="who" style="color:{colors[r.idx]}; width:{nameWidth + 1}ch">{names[r.idx]}</span>{#each decorate(r.line.segments, r.line.plain, queryRe) as p, i (i)}<span style={p.style} class:match={p.match}>{p.text}</span>{/each}</div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
  {#if allSelected}
    <div class="sel-hint" role="status">
      전체 선택 {rows.length.toLocaleString()}줄 · <b>Ctrl+C</b> 복사 · <b>Esc</b> 해제
      <button class="mini-btn" onclick={copySelection}>복사</button>
    </div>
  {/if}
</div>

<style>
  .merged {
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
    gap: 6px;
    height: 34px;
    padding: 0 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel-2);
    overflow: hidden;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 0 1 220px;
    min-width: 120px;
    height: 24px;
    padding: 0 6px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--muted);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 12px;
  }
  .chips {
    display: flex;
    gap: 4px;
    overflow: hidden;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: none;
    color: var(--text-2);
    font: inherit;
    font-size: 11.5px;
    white-space: nowrap;
    cursor: pointer;
  }
  .chip .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  .chip.off {
    opacity: 0.4;
  }
  .spacer {
    flex: 1;
  }
  .count {
    color: var(--faint);
    font-size: 11.5px;
    white-space: nowrap;
  }
  .tool {
    display: grid;
    place-items: center;
    width: 26px;
    height: 24px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .tool.on {
    color: var(--accent-text);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .viewport.all-selected .line {
    background: var(--console-selection);
  }
  .sel-hint {
    position: absolute;
    top: 42px;
    right: 20px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 6px 5px 12px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--panel);
    color: var(--muted);
    font-size: 12px;
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.35);
  }
  .sel-hint b {
    color: var(--text);
    font-weight: 600;
  }
  .mini-btn {
    padding: 2px 10px;
    border: 0;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font: inherit;
    cursor: pointer;
  }
  .viewport {
    flex: 1;
    min-height: 0;
    overflow: auto;
    outline: none;
    font-family: var(--font-mono);
    font-size: var(--console-font-size);
    line-height: var(--console-row);
    color: var(--console-fg);
    tab-size: 4;
  }
  .spacer-box {
    position: relative;
    width: max-content;
    min-width: 100%;
  }
  .line {
    height: var(--console-row);
    padding: 0 12px;
    white-space: pre;
  }
  .line.s1 {
    color: var(--console-err);
  }
  .line.s2 {
    color: var(--console-sys);
  }
  .line.lv3 {
    background: color-mix(in srgb, var(--err) 9%, transparent);
  }
  .line.lv2 {
    background: color-mix(in srgb, var(--warn) 8%, transparent);
  }
  .who {
    display: inline-block;
    margin-right: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    vertical-align: top;
    font-weight: 600;
  }
  .match {
    background: color-mix(in srgb, var(--warn) 45%, transparent);
    border-radius: 2px;
  }
  .placeholder {
    padding: 40px 24px;
    font-family: var(--font-ui);
    color: var(--muted);
    text-align: center;
  }
</style>
