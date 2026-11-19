<script lang="ts">
  import { tick } from 'svelte'
  import type { EnvMap } from '../types'
  import { isSortedByName, moveTo, sortByName, step } from '../reorder'
  import Icon from './Icon.svelte'

  let {
    value,
    onchange,
    resetKey,
    keyPlaceholder = '이름',
    valuePlaceholder = '값',
    addLabel = '추가',
  }: {
    value: EnvMap | undefined
    onchange: (v: EnvMap) => void
    /** 이 값이 바뀌면 입력 행을 value로 다시 채운다 (편집 대상 전환) */
    resetKey: unknown
    keyPlaceholder?: string
    valuePlaceholder?: string
    addLabel?: string
  } = $props()

  /** id: 순서를 바꿔도 같은 입력칸(포커스·보기 상태)이 행을 따라가게 하는 키 */
  type Row = { id: number; k: string; v: string; reveal: boolean }
  let rows = $state<Row[]>([])
  let container: HTMLDivElement | undefined = $state()
  let lastKey: unknown = Symbol('init')
  let nextId = 0

  const SENSITIVE = /PASS(WORD|WD)?|SECRET|TOKEN|API_?KEY|PRIVATE|CREDENTIAL/i
  /** 다른 입력칸에 글자로 떨어지지 않게 전용 형식으로만 싣는다 */
  const DRAG_TYPE = 'application/x-services-dashboard-row'

  $effect.pre(() => {
    if (resetKey !== lastKey) {
      lastKey = resetKey
      rows = Object.entries(value ?? {}).map(([k, v]) => ({ id: nextId++, k, v, reveal: false }))
    }
  })

  function emit() {
    const map: EnvMap = {}
    for (const r of rows) {
      const k = r.k.trim()
      if (k) map[k] = r.v
    }
    onchange(map)
  }

  async function add() {
    rows.push({ id: nextId++, k: '', v: '', reveal: true })
    await tick()
    const keys = container?.querySelectorAll<HTMLInputElement>('input.key')
    keys?.[keys.length - 1]?.focus()
  }

  function remove(i: number) {
    rows.splice(i, 1)
    emit()
  }

  // ── 순서 바꾸기: 손잡이를 끌어서 놓기, Alt+↑/↓, 이름순 정렬 ──
  let dragFrom = $state<number | null>(null)
  /** 끼워 넣을 자리 (0 = 맨 앞, rows.length = 맨 뒤) */
  let dropAt = $state<number | null>(null)

  function onDragStart(e: DragEvent, i: number) {
    const dt = e.dataTransfer
    if (!dt) return
    dragFrom = i
    dt.effectAllowed = 'move'
    dt.setData(DRAG_TYPE, String(i))
    // 손잡이만이 아니라 행 전체가 끌려가는 모습으로
    const row = (e.currentTarget as HTMLElement).closest<HTMLElement>('.row')
    if (row) {
      const r = row.getBoundingClientRect()
      dt.setDragImage(row, e.clientX - r.left, e.clientY - r.top)
    }
  }

  function onDragOver(e: DragEvent, i: number) {
    if (dragFrom === null) return
    e.preventDefault()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const at = e.clientY > r.top + r.height / 2 ? i + 1 : i
    // 제자리(자기 위·아래)는 표시하지 않는다
    dropAt = at === dragFrom || at === dragFrom + 1 ? null : at
  }

  function onDrop(e: DragEvent) {
    if (dragFrom === null) return
    e.preventDefault()
    if (dropAt !== null && moveTo(rows, dragFrom, dropAt)) emit()
    endDrag()
  }

  function endDrag() {
    dragFrom = null
    dropAt = null
  }

  async function onRowKeydown(e: KeyboardEvent, i: number) {
    const grip = (e.target as HTMLElement).classList.contains('grip')
    if ((!e.altKey && !grip) || e.ctrlKey || e.metaKey || e.shiftKey) return
    if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return
    e.preventDefault()
    const at = step(rows, i, e.key === 'ArrowUp' ? -1 : 1)
    if (at === i) return
    emit()
    // 행 DOM은 그대로 옮겨지지만, 옮기면서 포커스가 풀리면 다시 잡는다
    const el = e.target as HTMLElement
    await tick()
    if (document.activeElement !== el) el.focus()
  }

  function sortRows() {
    if (sortByName(rows, (r) => r.k)) emit()
  }

  /** 이미 이름순이면 정렬 버튼을 끈다 (이름이 빈 행은 저장되지 않으므로 무시) */
  const sorted = $derived(isSortedByName(rows.map((r) => r.k).filter((k) => k.trim())))

  const duplicates = $derived.by(() => {
    const seen = new Set<string>()
    const dup = new Set<string>()
    for (const r of rows) {
      const k = r.k.trim().toUpperCase()
      if (!k) continue
      if (seen.has(k)) dup.add(k)
      seen.add(k)
    }
    return dup
  })
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="kv" bind:this={container} ondrop={onDrop} ondragover={(e) => dragFrom !== null && e.preventDefault()}>
  {#each rows as row, i (row.id)}
    {@const secret = SENSITIVE.test(row.k)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="row"
      class:dragging={dragFrom === i}
      class:drop-before={dropAt === i}
      class:drop-after={dropAt === rows.length && i === rows.length - 1}
      ondragover={(e) => onDragOver(e, i)}
      onkeydown={(e) => onRowKeydown(e, i)}
    >
      <button
        type="button"
        class="grip"
        draggable={rows.length > 1}
        disabled={rows.length < 2}
        title="끌어서 순서 바꾸기 (Alt+↑/↓)"
        aria-label="순서 바꾸기"
        ondragstart={(e) => onDragStart(e, i)}
        ondragend={endDrag}
      ><Icon name="grip" size={14} /></button>
      <input
        class="key"
        class:dup={duplicates.has(row.k.trim().toUpperCase())}
        placeholder={keyPlaceholder}
        spellcheck="false"
        bind:value={row.k}
        oninput={emit}
      />
      <span class="eq">=</span>
      <input
        class="val"
        type={secret && !row.reveal ? 'password' : 'text'}
        placeholder={valuePlaceholder}
        spellcheck="false"
        bind:value={row.v}
        oninput={emit}
      />
      {#if secret}
        <button type="button" class="icon-btn" title={row.reveal ? '값 숨기기' : '값 보기'} onclick={() => (row.reveal = !row.reveal)}>
          {row.reveal ? '숨김' : '보기'}
        </button>
      {/if}
      <button type="button" class="icon-btn" title="삭제" onclick={() => remove(i)}><Icon name="x" size={12} /></button>
    </div>
  {/each}
  {#if duplicates.size}
    <p class="warn">같은 이름이 여러 번 있습니다 (Windows는 대소문자를 구분하지 않음). 마지막 값이 쓰입니다.</p>
  {/if}
  <div class="actions">
    <button type="button" class="add" onclick={add}><Icon name="plus" size={12} /> {addLabel}</button>
    {#if rows.length > 1}
      <button type="button" class="add" disabled={sorted} title={sorted ? '이미 이름순입니다' : '이름(A→Z)순으로 정렬합니다. 저장해야 반영됩니다.'} onclick={sortRows}>이름순 정렬</button>
    {/if}
  </div>
</div>

<style>
  .kv {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .row.dragging {
    opacity: 0.4;
  }
  /* 놓일 자리 표시: 행 사이 간격(4px) 가운데에 선 */
  .row.drop-before::before,
  .row.drop-after::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    border-radius: 1px;
    background: var(--accent);
    pointer-events: none;
  }
  .row.drop-before::before {
    top: -3px;
  }
  .row.drop-after::after {
    bottom: -3px;
  }
  .grip {
    display: grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: none;
    color: var(--faint);
    cursor: grab;
  }
  .grip:hover:not(:disabled),
  .grip:focus-visible {
    background: var(--hover);
    color: var(--text);
  }
  .grip:active:not(:disabled) {
    cursor: grabbing;
  }
  .grip:disabled {
    opacity: 0.35;
    cursor: default;
  }
  input {
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 12px;
    outline: 0;
    min-width: 0;
  }
  input:focus {
    border-color: var(--accent);
  }
  .key {
    flex: 0 0 38%;
  }
  .key.dup {
    border-color: var(--warn);
  }
  .val {
    flex: 1;
  }
  .eq {
    color: var(--faint);
  }
  .icon-btn {
    display: grid;
    place-items: center;
    min-width: 24px;
    height: 24px;
    padding: 0 4px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }
  .icon-btn:hover {
    background: var(--hover);
    color: var(--text);
  }
  .add {
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 2px;
    padding: 3px 8px;
    border: 1px dashed var(--border-strong);
    border-radius: 5px;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .add:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--muted);
  }
  .add:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .warn {
    margin: 2px 0;
    color: var(--warn);
    font-size: 11.5px;
  }
</style>
