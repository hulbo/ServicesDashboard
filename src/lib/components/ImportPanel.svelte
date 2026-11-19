<script lang="ts">
  import { app } from '../state.svelte'
  import { errorMessage } from '../api'
  import { findRootVar, initialItems, normPath, type ImportItem, type PresetItem } from '../importing'
  import type { ScanResult, SourceKind } from '../types'
  import { KIND_LABELS, SOURCE_LABELS } from '../types'
  import Icon from './Icon.svelte'

  let root = $state(app.importRoot)
  let scanning = $state(false)
  let importing = $state(false)
  let scan = $state<ScanResult | null>(null)
  let items = $state<ImportItem[]>([])
  let presetItems = $state<PresetItem[]>([])
  let error = $state<string | null>(null)
  let useVar = $state(true)
  let varName = $state('')
  let showWarnings = $state(false)
  let expanded = $state<Record<string, boolean>>({})

  const ORDER: SourceKind[] = ['shared', 'intellij', 'vscode', 'npm', 'gradle', 'maven']

  const recentRoots = $derived([...new Set(app.workspace.services.map((s) => s.source?.root).filter((r): r is string => !!r))])
  const existingKeys = $derived(new Set(app.workspace.services.map((s) => s.source?.key).filter(Boolean)))
  const sections = $derived(
    ORDER.map((kind) => ({ kind, rows: items.map((item, index) => ({ item, index })).filter((r) => r.item.candidate.source === kind) })).filter(
      (s) => s.rows.length > 0,
    ),
  )
  const selectedCount = $derived(items.filter((i) => i.selected).length)
  const groupNames = $derived([...new Set([...app.workspace.groups.map((g) => g.name), ...items.map((i) => i.group).filter(Boolean)])])
  const existingVar = $derived(scan ? findRootVar(app.workspace, scan.root) : undefined)
  const varConflict = $derived.by(() => {
    if (!scan || !useVar || existingVar || !varName) return false
    const cur = app.workspace.variables?.[varName]
    return cur !== undefined && normPath(cur) !== normPath(scan.root)
  })
  const varInvalid = $derived(useVar && !existingVar && !/^[A-Za-z_][A-Za-z0-9_]*$/.test(varName))

  async function doScan(path = root) {
    const target = path.trim()
    if (!target) return
    root = target
    app.importRoot = target
    scanning = true
    error = null
    try {
      const result = await app.backend.scanProject(target)
      scan = result
      items = initialItems(app.workspace, result)
      presetItems = result.presets.map((preset) => ({ preset, selected: preset.members.length > 0 }))
      varName = result.suggestedVar
      expanded = {}
      showWarnings = false
    } catch (e) {
      scan = null
      items = []
      error = errorMessage(e)
    } finally {
      scanning = false
    }
  }

  async function pick() {
    const p = await app.backend.pickFolder(root || undefined)
    if (p) await doScan(p)
  }

  function setSection(kind: SourceKind, on: boolean) {
    for (const i of items) if (i.candidate.source === kind) i.selected = on
  }

  async function doImport() {
    if (!scan || !selectedCount || varConflict || varInvalid) return
    importing = true
    try {
      const ok = await app.importItems(
        $state.snapshot(scan) as ScanResult,
        $state.snapshot(items) as ImportItem[],
        useVar && !existingVar ? varName : null,
        $state.snapshot(presetItems) as PresetItem[],
      )
      if (ok) {
        scan = null
        items = []
      }
    } finally {
      importing = false
    }
  }
</script>

<div class="panel">
  <header class="head">
    <Icon name="import" size={18} />
    <h2>가져오기</h2>
    <span class="sub">IntelliJ 실행 구성 · package.json 스크립트 · Spring Boot 모듈</span>
  </header>

  <div class="scroll">
    <section class="pick">
      <p class="hint">프로젝트 폴더(.idea가 있는 루트)를 고르면 실행할 수 있는 서비스 후보를 찾습니다. 원본 파일은 읽기만 합니다.</p>
      <form class="row" onsubmit={(e) => { e.preventDefault(); void doScan() }}>
        <input class="path mono" bind:value={root} placeholder="C:/Project/my-project" spellcheck="false" />
        <button type="button" class="btn" onclick={pick}><Icon name="folder" size={14} /> 폴더 선택</button>
        <button type="submit" class="btn primary" disabled={!root.trim() || scanning}><Icon name="search" size={14} /> {scanning ? '찾는 중…' : '스캔'}</button>
      </form>
      {#if recentRoots.length && !scan}
        <div class="recent">
          <span>최근:</span>
          {#each recentRoots as r (r)}
            <button class="chip mono" onclick={() => doScan(r)}>{r}</button>
          {/each}
        </div>
      {/if}
      {#if error}<p class="error">{error}</p>{/if}
    </section>

    {#if scan}
      <section class="summary">
        <div class="counts">
          {#each ORDER as kind (kind)}
            {@const n = items.filter((i) => i.candidate.source === kind).length}
            {#if n}<span class="count"><b>{n}</b> {SOURCE_LABELS[kind]}</span>{/if}
          {/each}
          {#if items.length === 0}<span class="count">찾은 후보가 없습니다.</span>{/if}
        </div>
        {#if scan.warnings.length}
          <button class="warn-toggle" onclick={() => (showWarnings = !showWarnings)}>
            <Icon name="warning" size={13} /> 알림 {scan.warnings.length}건 {showWarnings ? '접기' : '보기'}
          </button>
          {#if showWarnings}
            <ul class="warnings">
              {#each scan.warnings as w, i (i)}<li>{w}</li>{/each}
            </ul>
          {/if}
        {/if}

        <div class="var">
          {#if existingVar}
            <span class="hint">작업 디렉터리를 기존 변수 <code>${'{'}var:{existingVar}{'}'}</code>로 바꿔 저장합니다.</span>
          {:else}
            <label class="check"><input type="checkbox" bind:checked={useVar} /> 프로젝트 경로를 변수로 저장</label>
            {#if useVar}
              <input class="var-name mono" bind:value={varName} spellcheck="false" />
              <span class="hint">= <span class="mono">{scan.root}</span> · 작업 디렉터리가 <code>${'{'}var:{varName}{'}'}/...</code>가 됩니다.</span>
            {/if}
          {/if}
        </div>
        {#if varConflict}<p class="error">변수 '{varName}'이(가) 이미 다른 경로로 정의돼 있습니다. 다른 이름을 쓰세요.</p>{/if}
        {#if varInvalid}<p class="error">변수 이름은 영문·숫자·밑줄만 쓸 수 있고 숫자로 시작할 수 없습니다.</p>{/if}
      </section>

      {#each sections as section (section.kind)}
        {@const allOn = section.rows.every((r) => r.item.selected)}
        <section class="list">
          <div class="list-head">
            <label class="check">
              <input type="checkbox" checked={allOn} onchange={(e) => setSection(section.kind, e.currentTarget.checked)} />
              <b>{SOURCE_LABELS[section.kind]}</b>
            </label>
            <span class="muted">{section.rows.filter((r) => r.item.selected).length} / {section.rows.length}</span>
          </div>
          {#each section.rows as { item, index } (item.candidate.key)}
            {@const c = item.candidate}
            {@const exists = existingKeys.has(c.key)}
            <div class="cand" class:off={!item.selected}>
              <div class="line1">
                <input type="checkbox" bind:checked={items[index].selected} aria-label="{c.service.name} 선택" />
                <input class="name" bind:value={items[index].name} spellcheck="false" aria-label="이름" />
                <input class="group" bind:value={items[index].group} list="import-groups" placeholder="그룹 없음" spellcheck="false" aria-label="그룹" />
                <span class="badge">{KIND_LABELS[c.service.type]}</span>
                {#if exists}<span class="badge exists" title="같은 원본에서 이미 가져온 서비스가 있습니다. 선택하면 실행 정의를 갱신합니다.">이미 있음 · 갱신</span>{/if}
                <button class="more" title="자세히" onclick={() => (expanded[c.key] = !expanded[c.key])}>
                  <Icon name={expanded[c.key] ? 'chevron-down' : 'chevron-right'} size={14} />
                </button>
              </div>
              <div class="line2">
                <span class="origin mono">{c.origin}</span>
                {#if c.commandError}
                  <span class="cmd err" title={c.commandError}>{c.commandError}</span>
                {:else if c.command}
                  <span class="cmd mono" title={c.command}>{c.command}</span>
                {/if}
              </div>
              {#if c.scripts.length > 1 && items[index].script !== undefined}
                <div class="opt">
                  <span>스크립트</span>
                  <select bind:value={items[index].script}>
                    {#each c.scripts as s (s)}<option value={s}>{s}</option>{/each}
                  </select>
                </div>
              {/if}
              {#if c.profiles.length && items[index].profiles !== undefined}
                <div class="opt">
                  <span>프로파일</span>
                  <input class="mono" bind:value={items[index].profiles} placeholder="비우면 기본 프로파일" spellcheck="false" />
                  <span class="muted">후보: {c.profiles.join(', ')}</span>
                </div>
              {/if}
              {#each c.warnings as w, i (i)}
                <p class="cwarn"><Icon name="warning" size={12} /> {w}</p>
              {/each}
              {#if expanded[c.key]}
                <dl class="details">
                  <dt>작업 디렉터리</dt><dd class="mono">{c.service.cwd ?? '-'}</dd>
                  {#if c.service.mainClass}<dt>메인 클래스</dt><dd class="mono">{c.service.mainClass}</dd>{/if}
                  {#if c.service.runtime?.jdk}<dt>JDK</dt><dd class="mono">{c.service.runtime.jdk}</dd>{/if}
                  {#if c.service.runtime?.node}<dt>Node</dt><dd class="mono">{c.service.runtime.node}</dd>{/if}
                  {#if c.service.jvmArgs?.length}<dt>VM 옵션</dt><dd class="mono">{c.service.jvmArgs.join(' ')}</dd>{/if}
                  {#if c.service.env && Object.keys(c.service.env).length}
                    <dt>환경변수</dt><dd class="mono">{Object.keys(c.service.env).join(', ')}</dd>
                  {/if}
                  {#if c.command}<dt>실행 명령</dt><dd class="mono wrap">{c.command}</dd>{/if}
                </dl>
              {/if}
            </div>
          {/each}
        </section>
      {/each}
      {#if presetItems.length}
        <section class="list">
          <div class="list-head"><b>실행 프리셋 (복합 구성)</b><span class="muted">고른 서비스 중 가져온 것만 묶습니다</span></div>
          {#each presetItems as pi, i (pi.preset.key)}
            <div class="cand" class:off={!pi.selected}>
              <div class="line1">
                <input type="checkbox" bind:checked={presetItems[i].selected} disabled={!pi.preset.members.length} aria-label="{pi.preset.name} 프리셋 선택" />
                <span class="pname">{pi.preset.name}</span>
                <span class="badge">프리셋</span>
                <span class="origin mono">{pi.preset.origin}</span>
              </div>
              <div class="line2">
                <span class="cmd">
                  {pi.preset.members.map((k) => items.find((x) => x.candidate.key === k)?.name ?? k).join(', ') || '연결할 서비스 없음'}
                </span>
              </div>
              {#if pi.preset.missing.length}
                <p class="cwarn"><Icon name="warning" size={12} /> 찾지 못한 구성: {pi.preset.missing.join(', ')}</p>
              {/if}
            </div>
          {/each}
        </section>
      {/if}
      <datalist id="import-groups">
        {#each groupNames as g (g)}<option value={g}></option>{/each}
      </datalist>
    {/if}
  </div>

  {#if scan && items.length}
    <footer class="actions">
      <span class="muted">{selectedCount}개 선택</span>
      <span class="spacer"></span>
      <button class="btn" onclick={() => { scan = null; items = [] }}>취소</button>
      <button class="btn primary" disabled={!selectedCount || importing || varConflict || varInvalid} onclick={doImport}>
        <Icon name="import" size={14} /> {importing ? '가져오는 중…' : `${selectedCount}개 가져오기`}
      </button>
    </footer>
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
    color: var(--muted);
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .sub {
    font-size: 12px;
    color: var(--faint);
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 4px 20px 24px;
  }
  section {
    max-width: 980px;
    padding: 14px 0;
    border-bottom: 1px solid var(--border);
  }
  .hint {
    margin: 0 0 10px;
    color: var(--faint);
    font-size: 12px;
  }
  .muted {
    color: var(--faint);
    font-size: 12px;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  code {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-2);
  }
  .row {
    display: flex;
    gap: 6px;
  }
  input,
  select {
    height: 28px;
    box-sizing: border-box;
    padding: 0 8px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
    outline: 0;
    min-width: 0;
  }
  input:focus,
  select:focus {
    border-color: var(--accent);
  }
  input[type='checkbox'] {
    height: auto;
    accent-color: var(--accent);
  }
  .path {
    flex: 1;
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
    white-space: nowrap;
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
  .recent {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    color: var(--faint);
    font-size: 12px;
  }
  .chip {
    padding: 2px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .chip:hover {
    color: var(--text);
  }
  .error {
    margin: 8px 0 0;
    color: var(--err);
    font-size: 12.5px;
  }
  .counts {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    margin-bottom: 8px;
  }
  .count {
    color: var(--muted);
  }
  .count b {
    color: var(--text);
  }
  .warn-toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 0;
    border: 0;
    background: none;
    color: var(--warn);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .warnings {
    margin: 6px 0 4px;
    padding-left: 18px;
    color: var(--warn);
    font-size: 12px;
    line-height: 1.6;
  }
  .var {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
  .var .hint {
    margin: 0;
  }
  .var-name {
    width: 180px;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .cand {
    padding: 8px 10px;
    margin-bottom: 6px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--panel-2);
  }
  .cand.off {
    opacity: 0.6;
  }
  .line1 {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .name {
    flex: 1 1 200px;
  }
  .pname {
    flex: 1;
    font-weight: 600;
  }
  .group {
    flex: 0 1 140px;
  }
  .badge {
    flex: none;
    padding: 1px 7px;
    border-radius: 4px;
    background: var(--hover);
    color: var(--muted);
    font-size: 11.5px;
    white-space: nowrap;
  }
  .badge.exists {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent-text);
  }
  .more {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .more:hover {
    background: var(--hover);
  }
  .line2 {
    display: flex;
    gap: 10px;
    margin: 6px 0 0 24px;
    min-width: 0;
  }
  .origin {
    flex: none;
    color: var(--faint);
  }
  .cmd {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
  }
  .cmd.err {
    color: var(--err);
    font-size: 12px;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 0 24px;
    font-size: 12px;
    color: var(--muted);
  }
  .opt input {
    width: 220px;
  }
  .cwarn {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 5px 0 0 24px;
    color: var(--warn);
    font-size: 12px;
  }
  .details {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr);
    gap: 4px 10px;
    margin: 8px 0 0 24px;
    font-size: 12px;
  }
  .details dt {
    color: var(--faint);
  }
  .details dd {
    margin: 0;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .details dd.wrap {
    white-space: pre-wrap;
    word-break: break-all;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 20px;
    border-top: 1px solid var(--border);
    background: var(--panel);
  }
  .spacer {
    flex: 1;
  }
</style>
