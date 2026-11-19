<script lang="ts">
  import { tick } from 'svelte'
  import type { EnvMap } from '../types'
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

  type Row = { k: string; v: string; reveal: boolean }
  let rows = $state<Row[]>([])
  let container: HTMLDivElement | undefined = $state()
  let lastKey: unknown = Symbol('init')

  const SENSITIVE = /PASS(WORD|WD)?|SECRET|TOKEN|API_?KEY|PRIVATE|CREDENTIAL/i

  $effect.pre(() => {
    if (resetKey !== lastKey) {
      lastKey = resetKey
      rows = Object.entries(value ?? {}).map(([k, v]) => ({ k, v, reveal: false }))
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
    rows.push({ k: '', v: '', reveal: true })
    await tick()
    const keys = container?.querySelectorAll<HTMLInputElement>('input.key')
    keys?.[keys.length - 1]?.focus()
  }

  function remove(i: number) {
    rows.splice(i, 1)
    emit()
  }

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

<div class="kv" bind:this={container}>
  {#each rows as row, i (i)}
    {@const secret = SENSITIVE.test(row.k)}
    <div class="row">
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
  <button type="button" class="add" onclick={add}><Icon name="plus" size={12} /> {addLabel}</button>
</div>

<style>
  .kv {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
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
  .add:hover {
    color: var(--text);
    border-color: var(--muted);
  }
  .warn {
    margin: 2px 0;
    color: var(--warn);
    font-size: 11.5px;
  }
</style>
