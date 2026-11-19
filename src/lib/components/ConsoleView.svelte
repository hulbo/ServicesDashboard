<script lang="ts">
  import { tick, untrack } from 'svelte'
  import { app } from '../state.svelte'
  import { errorMessage } from '../api'
  import { buildRows, compileQuery, decorate, levelCounts, LEVEL_ERROR, LEVEL_WARN, rowLineCount, viewText, type Level, type ViewOptions } from '../console-view'
  import Icon from './Icon.svelte'

  let { serviceId }: { serviceId: string } = $props()

  const ROW = 18
  const OVERSCAN = 30

  let viewport: HTMLDivElement | undefined = $state()
  let searchInput: HTMLInputElement | undefined = $state()
  let scrollTop = $state(0)
  let height = $state(400)
  let follow = $state(true)
  let seenSeq = $state(0)
  let lastRemoved = 0
  let lastId = ''

  // 보기 옵션
  let query = $state('')
  let useRegex = $state(false)
  let caseSensitive = $state(false)
  let filterMode = $state(false)
  let levels = $state<Level[]>([])
  let foldFrames = $state(true)
  let showTime = $state(false)
  let expanded = $state<number[]>([])
  let cursor = $state(-1)

  const store = $derived(app.logStore(serviceId))
  const version = $derived(app.logTick[serviceId] ?? 0)
  const compiled = $derived(compileQuery(query, useRegex, caseSensitive))
  const queryRe = $derived(compiled instanceof RegExp ? compiled : null)
  const opts = $derived<ViewOptions>({ query: queryRe, filterMode, levels: new Set(levels), foldFrames, expanded: new Set(expanded) })
  const view = $derived.by(() => {
    void version
    return buildRows(store.lines, opts)
  })
  /** Ctrl+A: 콘솔 내용 전체 선택 (보이는 줄만 그리므로 브라우저 선택 대신 직접 표시·복사한다) */
  let allSelected = $state(false)
  const selectedCount = $derived(allSelected ? rowLineCount(view.rows) : 0)
  const counts = $derived.by(() => {
    void version
    return levelCounts(store.lines)
  })
  /** store는 반응형이 아니므로 version을 거쳐 줄 수를 읽는다 */
  const lineCount = $derived.by(() => {
    void version
    return store.lines.length
  })
  const total = $derived(view.rows.length)
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN))
  const last = $derived(Math.min(total, Math.ceil((scrollTop + height) / ROW) + OVERSCAN))
  const visible = $derived(view.rows.slice(first, last).map((row, i) => ({ row, index: first + i })))
  const currentRow = $derived(cursor >= 0 && cursor < view.matches.length ? view.matches[cursor] : -1)
  const unseen = $derived.by(() => {
    void version
    return follow ? 0 : Math.max(0, store.lastSeq - seenSeq)
  })

  // 서비스를 바꾸면 맨 아래부터 보여 준다.
  $effect.pre(() => {
    if (serviceId !== lastId) {
      lastId = serviceId
      follow = true
      expanded = []
      lastRemoved = store.removedTotal
      void app.ensureLogs(serviceId)
    }
  })

  // 새 로그: 따라가는 중이면 맨 아래로, 아니면 잘려 나간 줄만큼 위치를 보정한다.
  $effect(() => {
    void version
    const el = viewport
    if (!el) return
    const removed = store.removedTotal - lastRemoved
    lastRemoved = store.removedTotal
    if (untrack(() => follow)) {
      seenSeq = store.lastSeq
      void tick().then(() => {
        el.scrollTop = el.scrollHeight
        scrollTop = el.scrollTop
      })
    } else if (removed > 0 && !untrack(() => query)) {
      el.scrollTop = Math.max(0, el.scrollTop - removed * ROW)
      scrollTop = el.scrollTop
    }
  })

  // 검색어가 바뀌면 가장 최근 일치로 이동
  $effect(() => {
    void queryRe
    void filterMode
    const n = untrack(() => view.matches.length)
    if (!queryRe) {
      cursor = -1
      return
    }
    if (n) void goTo(n - 1)
    else cursor = -1
  })

  async function goTo(i: number) {
    const n = view.matches.length
    if (!n) return
    cursor = ((i % n) + n) % n
    follow = false
    await tick()
    const el = viewport
    if (!el) return
    el.scrollTop = Math.max(0, view.matches[cursor] * ROW - el.clientHeight / 2)
    scrollTop = el.scrollTop
  }

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

  function toggleLevel(l: Level) {
    levels = levels.includes(l) ? levels.filter((x) => x !== l) : [...levels, l]
  }

  function formatTime(ts: number) {
    const d = new Date(ts)
    const p = (n: number, w = 2) => String(n).padStart(w, '0')
    return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`
  }

  async function copyAll() {
    try {
      await navigator.clipboard.writeText(store.text())
      app.toast('success', `${store.lines.length.toLocaleString()}줄을 클립보드에 복사했습니다.`)
    } catch (e) {
      app.toast('error', `복사 실패: ${errorMessage(e)}`)
    }
  }

  async function saveToFile() {
    const name = app.serviceName(serviceId).replace(/[\\/:*?"<>|]+/g, '_')
    const stamp = new Date().toISOString().slice(0, 19).replace(/[-:T]/g, '')
    const path = await app.backend.pickSaveFile(`${name}-${stamp}.log`)
    if (!path) return
    try {
      const n = await app.backend.logsExport(serviceId, path)
      app.toast('success', `${n.toLocaleString()}줄을 저장했습니다: ${path}`)
    } catch (e) {
      app.toast('error', `저장 실패: ${errorMessage(e)}`)
    }
  }

  function selectAll() {
    allSelected = true
    window.getSelection()?.removeAllRanges()
    viewport?.focus()
  }

  function selectionText() {
    return viewText(store.lines, opts, showTime ? formatTime : undefined)
  }

  async function copySelection() {
    const { text, count } = selectionText()
    try {
      await navigator.clipboard.writeText(text)
      app.toast('success', `${count.toLocaleString()}줄을 클립보드에 복사했습니다.`)
    } catch (e) {
      app.toast('error', `복사 실패: ${errorMessage(e)}`)
    }
  }

  function onCopy(e: ClipboardEvent) {
    // 우클릭 메뉴 '복사' 등 키보드가 아닌 복사도 전체 선택이면 콘솔 내용을 넣는다
    if (!allSelected || !e.clipboardData) return
    e.preventDefault()
    e.clipboardData.setData('text/plain', selectionText().text)
  }

  function onKeydown(e: KeyboardEvent) {
    const editing = (e.target as HTMLElement | null)?.closest('input, textarea, select')
    if (e.ctrlKey && !e.altKey && (e.key === 'a' || e.key === 'A') && !editing) {
      // 화면 전체(트리·버튼 글자까지)가 아니라 이 콘솔의 내용만 선택한다
      e.preventDefault()
      selectAll()
    } else if (allSelected && e.ctrlKey && (e.key === 'c' || e.key === 'C' || e.key === 'Insert') && !editing) {
      e.preventDefault()
      void copySelection()
    } else if (allSelected && e.key === 'Escape') {
      allSelected = false
    } else if (e.ctrlKey && (e.key === 'l' || e.key === 'L')) {
      e.preventDefault()
      void app.clearLogs(serviceId)
    } else if (e.ctrlKey && (e.key === 'f' || e.key === 'F')) {
      e.preventDefault()
      searchInput?.focus()
      searchInput?.select()
    } else if (e.key === 'End' && e.ctrlKey) {
      e.preventDefault()
      toBottom()
    }
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      void goTo(cursor + (e.shiftKey ? -1 : 1))
    } else if (e.key === 'Escape') {
      query = ''
      filterMode = false
      viewport?.focus()
    }
  }

  function openLink(e: MouseEvent, href: string) {
    e.preventDefault()
    void app.backend.openUrl(href)
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="console" onkeydown={onKeydown}>
  <div class="bar">
    <label class="search" class:invalid={compiled === 'invalid'}>
      <Icon name="search" size={13} />
      <input bind:this={searchInput} bind:value={query} placeholder="검색 (Ctrl+F)" spellcheck="false" onkeydown={onSearchKey} aria-label="콘솔 검색" />
      {#if query}
        <span class="hits">{view.matches.length ? `${cursor + 1}/${view.matches.length}` : compiled === 'invalid' ? '정규식 오류' : '0'}</span>
        <button class="mini up" title="이전 (Shift+Enter)" disabled={!view.matches.length} onclick={() => goTo(cursor - 1)}><Icon name="chevron-down" size={12} /></button>
        <button class="mini" title="다음 (Enter)" disabled={!view.matches.length} onclick={() => goTo(cursor + 1)}><Icon name="chevron-down" size={12} /></button>
      {/if}
    </label>
    <button class="tog" class:on={caseSensitive} title="대소문자 구분" onclick={() => (caseSensitive = !caseSensitive)}>Aa</button>
    <button class="tog mono" class:on={useRegex} title="정규식" onclick={() => (useRegex = !useRegex)}>.*</button>
    <button class="tog" class:on={filterMode} title="검색어가 있는 줄만 보기" disabled={!query} onclick={() => (filterMode = !filterMode)}>필터</button>
    <span class="sep"></span>
    <button class="tog lv err" class:on={levels.includes(LEVEL_ERROR)} title="ERROR 줄만 보기 (스택트레이스 포함)" onclick={() => toggleLevel(LEVEL_ERROR)}>
      ERROR <b>{counts.error}</b>
    </button>
    <button class="tog lv warn" class:on={levels.includes(LEVEL_WARN)} title="WARN 줄만 보기" onclick={() => toggleLevel(LEVEL_WARN)}>
      WARN <b>{counts.warn}</b>
    </button>
    <span class="spacer"></span>
    <button class="tog" class:on={foldFrames} title="긴 스택트레이스 접기" onclick={() => (foldFrames = !foldFrames)}>접기</button>
    <button class="tog" class:on={showTime} title="수신 시각 표시" onclick={() => (showTime = !showTime)}>시각</button>
    <button class="tool" class:on={follow} title="자동 스크롤 (Ctrl+End)" onclick={() => (follow ? (follow = false) : toBottom())}><Icon name="arrow-down" size={14} /></button>
    <button class="tool" title="파일로 저장" disabled={lineCount === 0} onclick={saveToFile}><Icon name="file" size={14} /></button>
    <button class="tool" title="전체 복사" disabled={lineCount === 0} onclick={copyAll}><Icon name="copy" size={14} /></button>
    <button class="tool" title="콘솔 지우기 (Ctrl+L)" disabled={lineCount === 0} onclick={() => app.clearLogs(serviceId)}><Icon name="clear" size={14} /></button>
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
    aria-live="off"
  >
    {#if lineCount === 0}
      <div class="placeholder">
        <p>아직 로그가 없습니다.</p>
        <p class="sub">▶ 시작을 누르면 이곳에 콘솔 출력이 표시됩니다.</p>
      </div>
    {:else if total === 0}
      <div class="placeholder"><p>조건에 맞는 줄이 없습니다.</p></div>
    {:else}
      <div class="spacer-box" style="height:{total * ROW}px">
        <div class="rows" style="transform:translateY({first * ROW}px)">
          {#each visible as { row, index } (row.kind === 'line' ? row.line.seq : `f${row.key}`)}
            {#if row.kind === 'fold'}
              <button class="fold" onclick={() => (expanded = [...expanded, row.key])}>⋯ 스택트레이스 {row.count}줄 접힘 — 펼치기</button>
            {:else}
              {@const line = row.line}
              <div class="line s{line.stream} lv{line.level}" class:current={index === currentRow}>{#if showTime}<span class="ts">{formatTime(line.ts)}</span>{/if}{#each decorate(line.segments, line.plain, queryRe) as p, i (i)}{#if p.href}<a href={p.href} style={p.style} class:match={p.match} title="클릭하면 브라우저에서 엽니다" onclick={(e) => openLink(e, p.href!)}>{p.text}</a>{:else}<span style={p.style} class:match={p.match}>{p.text}</span>{/if}{/each}{#if line.segments.length === 0}&#8203;{/if}</div>
            {/if}
          {/each}
        </div>
      </div>
    {/if}
  </div>

  {#if allSelected}
    <div class="sel-hint" role="status">
      전체 선택 {selectedCount.toLocaleString()}줄 · <b>Ctrl+C</b> 복사 · <b>Esc</b> 해제
      <button class="mini-btn" onclick={copySelection}>복사</button>
    </div>
  {/if}

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
    gap: 3px;
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
    flex: 0 1 260px;
    min-width: 140px;
    height: 24px;
    padding: 0 6px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--muted);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search.invalid {
    border-color: var(--err);
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
  .hits {
    color: var(--faint);
    font-size: 11px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .mini {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .mini.up {
    transform: rotate(180deg);
  }
  .mini:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .tog,
  .tool {
    display: flex;
    flex: none;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 7px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 11.5px;
    white-space: nowrap;
    cursor: pointer;
  }
  .tool {
    padding: 0 5px;
  }
  .tog:hover:not(:disabled),
  .tool:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .tog.on,
  .tool.on {
    color: var(--accent-text);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .tog:disabled,
  .tool:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .tog b {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .lv.err b {
    color: var(--err);
  }
  .lv.warn b {
    color: var(--warn);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .sep {
    flex: none;
    width: 1px;
    height: 16px;
    margin: 0 3px;
    background: var(--border-strong);
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
    will-change: transform;
  }
  .line {
    height: 18px;
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
  .line.current {
    box-shadow: inset 3px 0 0 var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .ts {
    display: inline-block;
    margin-right: 10px;
    color: var(--faint);
  }
  .match {
    background: color-mix(in srgb, var(--warn) 45%, transparent);
    border-radius: 2px;
  }
  .line.current .match {
    background: var(--warn);
    color: #000;
  }
  a {
    color: inherit;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, currentColor 50%, transparent);
    cursor: pointer;
  }
  a:hover {
    text-decoration-color: currentColor;
  }
  .fold {
    display: block;
    height: 18px;
    margin: 0 12px;
    padding: 0 8px;
    border: 0;
    border-radius: 3px;
    background: var(--hover);
    color: var(--muted);
    font-family: var(--font-ui);
    font-size: 11.5px;
    line-height: 18px;
    text-align: left;
    cursor: pointer;
  }
  .fold:hover {
    color: var(--text);
  }
  .line ::selection,
  .line::selection {
    background: var(--console-selection);
  }
  .viewport.all-selected .line,
  .viewport.all-selected .fold {
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
