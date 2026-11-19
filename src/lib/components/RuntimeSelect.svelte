<script lang="ts">
  import { app } from '../state.svelte'
  import type { RuntimeKind } from '../types'
  import Icon from './Icon.svelte'

  let {
    kind,
    value,
    onchange,
    id,
  }: { kind: RuntimeKind; value: string | undefined; onchange: (v: string | undefined) => void; id?: string } = $props()

  const DEFAULT_LABEL: Record<RuntimeKind, string> = {
    jdk: '기본값 — 시스템 JAVA_HOME',
    node: '기본값 — PATH의 node',
    maven: '기본값 — Maven Wrapper(mvnw) 또는 PATH의 mvn',
  }
  const NAME: Record<RuntimeKind, string> = { jdk: 'JDK', node: 'Node', maven: 'Maven' }

  const list = $derived(app.runtimes.filter((r) => r.kind === kind))
  const majors = $derived([...new Set(list.map((r) => r.major))].sort((a, b) => Number(b) - Number(a)))
  const known = $derived(new Set([...list.map((r) => r.path), ...majors]))
  const current = $derived(value?.trim() ?? '')

  let customMode = $state(false)
  const showCustom = $derived(customMode || (current !== '' && !known.has(current) && !app.runtimesLoading))
  const selectValue = $derived(showCustom ? '__custom__' : current)

  function onSelect(v: string) {
    if (v === '__custom__') {
      customMode = true
      return
    }
    customMode = false
    onchange(v || undefined)
  }

  async function browse() {
    const p = await app.backend.pickFolder(current && !current.includes('${') ? current : undefined)
    if (p) {
      customMode = true
      onchange(p)
    }
  }
</script>

<div class="rt">
  <select {id} value={selectValue} onchange={(e) => onSelect(e.currentTarget.value)}>
    <option value="">{DEFAULT_LABEL[kind]}</option>
    {#if majors.length}
      <optgroup label="버전 별칭 (이 PC에 설치된 같은 주 버전 중 최신)">
        {#each majors as m (m)}
          <option value={m}>{NAME[kind]} {m}</option>
        {/each}
      </optgroup>
    {/if}
    {#if list.length}
      <optgroup label="감지된 설치">
        {#each list as r (r.path)}
          <option value={r.path}>{r.version} · {r.vendor ?? r.source} — {r.path}</option>
        {/each}
      </optgroup>
    {/if}
    <option value="__custom__">직접 입력…</option>
  </select>
  {#if showCustom}
    <div class="custom">
      <input
        value={current}
        placeholder={kind === 'node' ? 'node.exe가 있는 폴더 또는 별칭 (예: 20)' : `${NAME[kind]} 홈 폴더 또는 별칭 (예: 17)`}
        spellcheck="false"
        class="mono"
        onchange={(e) => onchange(e.currentTarget.value.trim() || undefined)}
      />
      <button type="button" class="btn-icon" title="폴더 선택" onclick={browse}><Icon name="folder" size={14} /></button>
    </div>
  {/if}
  {#if app.runtimesLoading}<p class="hint">설치된 {NAME[kind]}를 찾는 중…</p>{/if}
</div>

<style>
  .rt {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  select,
  input {
    width: 100%;
    height: 28px;
    box-sizing: border-box;
    padding: 0 8px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
    outline: 0;
  }
  select:focus,
  input:focus {
    border-color: var(--accent);
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .custom {
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
  .hint {
    margin: 0;
    color: var(--faint);
    font-size: 11.5px;
  }
</style>
