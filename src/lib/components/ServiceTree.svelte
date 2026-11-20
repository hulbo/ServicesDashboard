<script lang="ts">
  import { app } from '../state.svelte'
  import { errorMessage } from '../api'
  import type { Group, Preset, Service } from '../types'
  import { DEBUG_KINDS, isActive } from '../types'
  import Icon, { type IconName } from './Icon.svelte'
  import { toolbarFit } from '../toolbar'
  import StatusDot from './StatusDot.svelte'
  import ContextMenu, { type MenuEntry } from './ContextMenu.svelte'

  let filter = $state('')
  let menu = $state<{ x: number; y: number; items: MenuEntry[] } | null>(null)

  // ── 도구 모음: 사이드바가 좁으면 오른쪽 도구부터 "더 보기(⋯)" 메뉴로 ──
  type Tool = { label: string; title: string; icon: IconName; group: number; action: () => void; active?: boolean; disabled?: boolean; danger?: boolean }
  const tools = $derived<Tool[]>([
    { label: '서비스 추가', title: '서비스 추가', icon: 'plus', group: 0, action: () => app.addService(app.selection?.kind === 'group' ? app.selection.id : undefined) },
    { label: '그룹 추가', title: '그룹 추가', icon: 'folder-plus', group: 0, action: () => app.addGroup() },
    { label: '가져오기', title: '가져오기 (IntelliJ·package.json·Spring Boot)', icon: 'import', group: 0, active: app.selection?.kind === 'import', action: () => app.select({ kind: 'import' }) },
    { label: '브랜치', title: '브랜치 (git 브랜치 보기·전환)', icon: 'branch', group: 0, active: app.selection?.kind === 'branches', action: () => app.openBranches() },
    { label: '전체 중지', title: '실행 중인 서비스 모두 중지', icon: 'stop-all', group: 1, danger: true, disabled: app.runningCount === 0, action: () => app.stopAll() },
    { label: '런타임', title: '런타임 (JDK·Node·Maven)', icon: 'runtime', group: 2, active: app.selection?.kind === 'runtimes', action: () => app.select({ kind: 'runtimes' }) },
    { label: '설정', title: '설정 (글꼴·트레이·알림·로그 파일)', icon: 'gear', group: 2, active: app.selection?.kind === 'settings', action: () => app.select({ kind: 'settings' }) },
    { label: '워크스페이스 설정', title: '워크스페이스 설정 (변수·공통 환경변수)', icon: 'workspace', group: 2, active: app.selection?.kind === 'workspace', action: () => app.select({ kind: 'workspace' }) },
  ])
  let toolbarWidth = $state(0)
  /** 바로 보일 도구 수 (toolbar 좌우 여백 16px 제외) */
  const shownCount = $derived(toolbarWidth ? toolbarFit(tools.map((t) => t.group), toolbarWidth - 16) : tools.length)
  const shown = $derived(tools.slice(0, shownCount))
  const hidden = $derived(tools.slice(shownCount))

  function openMore(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const items: MenuEntry[] = []
    hidden.forEach((t, i) => {
      if (i > 0 && t.group !== hidden[i - 1].group) items.push('separator')
      items.push({ label: t.label, icon: t.icon, disabled: t.disabled, danger: t.danger && !t.disabled, action: t.action })
    })
    menu = { x: r.left, y: r.bottom + 2, items }
  }
  let drag = $state<{ kind: 'service' | 'group'; id: string } | null>(null)
  let dropTarget = $state<{ id: string; mode: 'before' | 'into' } | null>(null)
  let listEl: HTMLDivElement | undefined = $state()

  type Node =
    | { kind: 'group'; group: Group; services: Service[] }
    | { kind: 'service'; service: Service; depth: number }

  const query = $derived(filter.trim().toLowerCase())
  const matches = (s: Service) => !query || s.name.toLowerCase().includes(query) || s.type.includes(query)

  const tree = $derived.by(() => {
    const ws = app.workspace
    const groups = ws.groups.map((group) => ({
      group,
      services: ws.services.filter((s) => s.group === group.id && matches(s)),
    }))
    const known = new Set(ws.groups.map((g) => g.id))
    const ungrouped = ws.services.filter((s) => (!s.group || !known.has(s.group)) && matches(s))
    return { groups: query ? groups.filter((g) => g.services.length > 0) : groups, ungrouped }
  })

  /** 키보드 이동용으로 화면에 보이는 항목을 순서대로 펼친 목록 */
  const flat = $derived.by(() => {
    const out: Node[] = []
    for (const g of tree.groups) {
      out.push({ kind: 'group', group: g.group, services: g.services })
      if (!app.collapsed[g.group.id] || query) for (const s of g.services) out.push({ kind: 'service', service: s, depth: 1 })
    }
    for (const s of tree.ungrouped) out.push({ kind: 'service', service: s, depth: 0 })
    return out
  })

  function groupCounts(groupId: string) {
    const services = app.workspace.services.filter((s) => s.group === groupId)
    const running = services.filter((s) => isActive(app.statuses[s.id]?.state)).length
    return { running, total: services.length }
  }

  const presets = $derived(app.workspace.presets ?? [])

  function presetCounts(p: Preset) {
    const ids = p.services.filter((id) => app.workspace.services.some((s) => s.id === id))
    return { running: ids.filter((id) => isActive(app.statuses[id]?.state)).length, total: ids.length }
  }

  function presetMenu(e: MouseEvent, p: Preset) {
    e.preventDefault()
    app.select({ kind: 'preset', id: p.id })
    const c = presetCounts(p)
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: '프리셋 전체 시작', icon: 'play-all', disabled: c.total === 0, action: () => app.startPreset(p.id) },
        { label: '프리셋 전체 중지', icon: 'stop-all', disabled: c.running === 0, action: () => app.stopPreset(p.id) },
        'separator',
        { label: '이름 바꾸기', icon: 'edit', action: () => (app.renaming = p.id) },
        { label: '프리셋 삭제', icon: 'trash', danger: true, action: () => app.deletePreset(p.id) },
      ],
    }
  }

  function isSelected(kind: 'service' | 'group' | 'preset', id: string) {
    const sel = app.selection
    return !!sel && sel.kind === kind && 'id' in sel && sel.id === id
  }

  async function openCwd(svc: Service) {
    try {
      const p = await app.backend.preview($state.snapshot(svc) as Service)
      await app.backend.openPath(p.cwd)
    } catch (e) {
      app.toast('error', errorMessage(e))
    }
  }

  async function openLogDir(id: string) {
    try {
      await app.backend.openPath(await app.backend.logsDir(id))
    } catch (e) {
      app.toast('error', errorMessage(e))
    }
  }

  function serviceMenu(e: MouseEvent, svc: Service) {
    e.preventDefault()
    app.select({ kind: 'service', id: svc.id })
    const active = isActive(app.statuses[svc.id]?.state)
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        active
          ? { label: '중지', icon: 'stop', shortcut: 'Ctrl+F2', action: () => app.stop(svc.id) }
          : { label: '시작', icon: 'play', shortcut: 'Shift+F10', action: () => app.start(svc.id) },
        { label: '재시작', icon: 'restart', shortcut: 'Ctrl+F5', disabled: !active, action: () => app.restart(svc.id) },
        ...(DEBUG_KINDS.includes(svc.type) && !active
          ? ([{ label: '디버그로 시작', icon: 'bug', shortcut: 'Shift+F9', action: () => app.start(svc.id, true) }] as MenuEntry[])
          : []),
        'separator',
        { label: '콘솔 보기', icon: 'terminal', action: () => (app.tab = 'console') },
        { label: '설정 편집', icon: 'gear', action: () => (app.tab = 'settings') },
        { label: '이름 바꾸기', icon: 'edit', shortcut: 'F2', action: () => (app.renaming = svc.id) },
        { label: '복제', icon: 'copy', action: () => app.duplicateService(svc.id) },
        'separator',
        { label: '작업 디렉터리 열기', icon: 'folder', action: () => openCwd(svc) },
        ...(app.settings?.logFiles.enabled
          ? ([{ label: '로그 파일 폴더 열기', icon: 'file', action: () => openLogDir(svc.id) }] as MenuEntry[])
          : []),
        ...(svc.source?.key
          ? ([{ label: '원본에서 다시 가져오기', icon: 'refresh', action: () => app.resync(svc.id) }] as MenuEntry[])
          : []),
        'separator',
        { label: '삭제', icon: 'trash', shortcut: 'Delete', danger: true, action: () => app.deleteService(svc.id) },
      ],
    }
  }

  function groupMenu(e: MouseEvent, group: Group) {
    e.preventDefault()
    app.select({ kind: 'group', id: group.id })
    const c = groupCounts(group.id)
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: '그룹 전체 시작', icon: 'play-all', disabled: c.total === 0, action: () => app.startGroup(group.id) },
        { label: '그룹 전체 중지', icon: 'stop-all', disabled: c.running === 0, action: () => app.stopGroup(group.id) },
        'separator',
        { label: '이 그룹에 서비스 추가', icon: 'plus', action: () => app.addService(group.id) },
        { label: '이름 바꾸기', icon: 'edit', shortcut: 'F2', action: () => (app.renaming = group.id) },
        'separator',
        { label: '그룹 삭제', icon: 'trash', danger: true, action: () => app.deleteGroup(group.id) },
      ],
    }
  }

  function backgroundMenu(e: MouseEvent) {
    if (e.target !== e.currentTarget) return
    e.preventDefault()
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: '서비스 추가', icon: 'plus', action: () => app.addService() },
        { label: '그룹 추가', icon: 'folder-plus', action: () => app.addGroup() },
        { label: '프리셋 추가', icon: 'play-all', action: () => app.addPreset() },
        { label: '가져오기…', icon: 'import', action: () => app.select({ kind: 'import' }) },
      ],
    }
  }

  // ---------- 키보드 ----------

  function currentIndex() {
    const sel = app.selection
    if (!sel || !('id' in sel)) return -1
    return flat.findIndex((n) => (n.kind === 'group' ? sel.kind === 'group' && n.group.id === sel.id : sel.kind === 'service' && n.service.id === sel.id))
  }

  function selectNode(n: Node | undefined) {
    if (!n) return
    if (n.kind === 'group') app.select({ kind: 'group', id: n.group.id })
    else app.select({ kind: 'service', id: n.service.id })
    queueMicrotask(() => listEl?.querySelector('.row.selected')?.scrollIntoView({ block: 'nearest' }))
  }

  function onKeydown(e: KeyboardEvent) {
    if (app.renaming || (e.target as HTMLElement).tagName === 'INPUT') return
    const i = currentIndex()
    const node = flat[i]
    if (e.key === 'ArrowDown') {
      selectNode(flat[Math.min(flat.length - 1, i + 1)])
    } else if (e.key === 'ArrowUp') {
      selectNode(flat[Math.max(0, i - 1)])
    } else if (e.key === 'ArrowLeft' && node?.kind === 'group' && !app.collapsed[node.group.id]) {
      app.toggleCollapsed(node.group.id)
    } else if (e.key === 'ArrowRight' && node?.kind === 'group' && app.collapsed[node.group.id]) {
      app.toggleCollapsed(node.group.id)
    } else if (e.key === 'Enter' && node?.kind === 'service') {
      app.toggle(node.service.id)
    } else if (e.key === 'F2' && node) {
      app.renaming = node.kind === 'group' ? node.group.id : node.service.id
    } else if (e.key === 'Delete' && node) {
      if (node.kind === 'group') app.deleteGroup(node.group.id)
      else app.deleteService(node.service.id)
    } else {
      return
    }
    e.preventDefault()
  }

  // ---------- 드래그 앤 드롭 ----------

  function onDragStart(e: DragEvent, kind: 'service' | 'group', id: string) {
    drag = { kind, id }
    e.dataTransfer?.setData('text/plain', id)
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
  }

  function onDragOver(e: DragEvent, targetKind: 'service' | 'group' | 'root', id: string) {
    if (!drag) return
    // 그룹은 그룹 위·루트에만, 서비스는 어디든 놓을 수 있다.
    if (drag.kind === 'group' && targetKind === 'service') return
    e.preventDefault()
    e.stopPropagation()
    const mode = targetKind === 'group' && drag.kind === 'service' ? 'into' : 'before'
    dropTarget = { id, mode }
  }

  function onDrop(e: DragEvent, targetKind: 'service' | 'group' | 'root', target?: Service | Group) {
    e.preventDefault()
    e.stopPropagation()
    const d = drag
    drag = null
    dropTarget = null
    if (!d) return
    if (d.kind === 'service') {
      if (targetKind === 'service') {
        const t = target as Service
        app.moveService(d.id, t.group ?? null, t.id)
      } else if (targetKind === 'group') {
        app.moveService(d.id, (target as Group).id)
        app.collapsed[(target as Group).id] = false
      } else {
        app.moveService(d.id, null)
      }
    } else if (targetKind === 'group') {
      app.moveGroup(d.id, (target as Group).id)
    } else if (targetKind === 'root') {
      app.moveGroup(d.id)
    }
  }

  function onDragEnd() {
    drag = null
    dropTarget = null
  }

  function renameKey(e: KeyboardEvent, id: string) {
    const input = e.currentTarget as HTMLInputElement
    if (e.key === 'Enter') app.rename(id, input.value)
    else if (e.key === 'Escape') app.renaming = null
    e.stopPropagation()
  }

  function focusSelect(node: HTMLInputElement) {
    node.focus()
    node.select()
  }
</script>

<div class="tree-panel">
  <div class="toolbar" bind:clientWidth={toolbarWidth}>
    {#each shown as t, i (t.label)}
      {#if i > 0 && t.group !== shown[i - 1].group}
        {#if t.group === 1}<span class="sep"></span>{:else}<span class="spacer"></span>{/if}
      {/if}
      <button class="tool" class:stop={t.danger} class:active={t.active} title={t.title} disabled={t.disabled} onclick={t.action}>
        <Icon name={t.icon} />
      </button>
    {/each}
    {#if hidden.length}
      <span class="spacer"></span>
      <button class="tool more" class:active={hidden.some((t) => t.active)} title="더 보기: {hidden.map((t) => t.label).join(' · ')}" aria-label="더 보기" onclick={openMore}>
        <Icon name="more" />
      </button>
    {/if}
  </div>

  <label class="filter">
    <Icon name="search" size={14} />
    <input placeholder="서비스 검색" bind:value={filter} spellcheck="false" onkeydown={(e) => e.key === 'Escape' && (filter = '')} />
    {#if filter}
      <button class="clear" title="지우기" onclick={() => (filter = '')}><Icon name="x" size={12} /></button>
    {/if}
  </label>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="list"
    role="tree"
    tabindex="0"
    bind:this={listEl}
    onkeydown={onKeydown}
    oncontextmenu={backgroundMenu}
    ondragover={(e) => onDragOver(e, 'root', '__root__')}
    ondrop={(e) => onDrop(e, 'root')}
  >
    {#if presets.length && !query}
      <div class="section-head">프리셋</div>
      {#each presets as p (p.id)}
        {@const c = presetCounts(p)}
        <div
          class="row preset"
          class:selected={isSelected('preset', p.id)}
          role="treeitem"
          aria-selected={isSelected('preset', p.id)}
          tabindex="-1"
          onclick={() => app.select({ kind: 'preset', id: p.id })}
          ondblclick={() => app.startPreset(p.id)}
          oncontextmenu={(e) => presetMenu(e, p)}
          onkeydown={() => {}}
        >
          <span class="pico"><Icon name="play-all" size={13} /></span>
          {#if app.renaming === p.id}
            <input class="rename" value={p.name} use:focusSelect onkeydown={(e) => renameKey(e, p.id)} onblur={(e) => app.rename(p.id, e.currentTarget.value)} onclick={(e) => e.stopPropagation()} />
          {:else}
            <span class="name">{p.name}</span>
            <span class="count" class:live={c.running > 0}>{c.running}/{c.total}</span>
            <span class="actions">
              {#if c.running > 0}
                <button class="act stop" title="프리셋 전체 중지" onclick={(e) => { e.stopPropagation(); app.stopPreset(p.id) }}><Icon name="stop" size={14} /></button>
              {/if}
              {#if c.running < c.total}
                <button class="act play" title="프리셋 전체 시작" onclick={(e) => { e.stopPropagation(); app.startPreset(p.id) }}><Icon name="play-all" size={14} /></button>
              {/if}
            </span>
          {/if}
        </div>
      {/each}
      <div class="divider"></div>
    {/if}

    {#each tree.groups as { group, services } (group.id)}
      {@const counts = groupCounts(group.id)}
      {@const open = !app.collapsed[group.id] || !!query}
      <div
        class="row group"
        class:selected={isSelected('group', group.id)}
        class:drop-before={dropTarget?.id === group.id && dropTarget.mode === 'before'}
        class:drop-into={dropTarget?.id === group.id && dropTarget.mode === 'into'}
        role="treeitem"
        aria-selected={isSelected('group', group.id)}
        aria-expanded={open}
        tabindex="-1"
        draggable={app.renaming !== group.id}
        ondragstart={(e) => onDragStart(e, 'group', group.id)}
        ondragover={(e) => onDragOver(e, 'group', group.id)}
        ondrop={(e) => onDrop(e, 'group', group)}
        ondragend={onDragEnd}
        onclick={() => app.select({ kind: 'group', id: group.id })}
        ondblclick={() => app.toggleCollapsed(group.id)}
        oncontextmenu={(e) => groupMenu(e, group)}
        onkeydown={() => {}}
      >
        <button class="twisty" tabindex="-1" onclick={(e) => { e.stopPropagation(); app.toggleCollapsed(group.id) }}>
          <Icon name={open ? 'chevron-down' : 'chevron-right'} size={14} />
        </button>
        <span class="folder"><Icon name="folder" size={15} /></span>
        {#if app.renaming === group.id}
          <input class="rename" value={group.name} use:focusSelect onkeydown={(e) => renameKey(e, group.id)} onblur={(e) => app.rename(group.id, e.currentTarget.value)} onclick={(e) => e.stopPropagation()} />
        {:else}
          <span class="name">{group.name}</span>
          <span class="count" class:live={counts.running > 0}>{counts.running}/{counts.total}</span>
          <span class="actions">
            {#if counts.running > 0}
              <button class="act stop" title="그룹 전체 중지" onclick={(e) => { e.stopPropagation(); app.stopGroup(group.id) }}><Icon name="stop" size={14} /></button>
            {/if}
            {#if counts.running < counts.total}
              <button class="act play" title="그룹 전체 시작" onclick={(e) => { e.stopPropagation(); app.startGroup(group.id) }}><Icon name="play-all" size={14} /></button>
            {/if}
          </span>
        {/if}
      </div>
      {#if open}
        {#each services as svc (svc.id)}
          {@render serviceRow(svc, 1)}
        {/each}
        {#if services.length === 0 && !query}
          <div class="empty-group">비어 있음 — 서비스를 끌어다 놓으세요</div>
        {/if}
      {/if}
    {/each}

    {#each tree.ungrouped as svc (svc.id)}
      {@render serviceRow(svc, 0)}
    {/each}

    {#if app.workspace.services.length === 0 && app.workspace.groups.length === 0}
      <div class="hint">
        <p>등록된 서비스가 없습니다.</p>
        <button class="primary" onclick={() => app.select({ kind: 'import' })}><Icon name="import" size={14} /> 프로젝트에서 가져오기</button>
        <button class="secondary" onclick={() => app.addService()}><Icon name="plus" size={14} /> 직접 추가</button>
      </div>
    {:else if query && tree.groups.length === 0 && tree.ungrouped.length === 0}
      <div class="hint"><p>'{filter}'와 일치하는 서비스가 없습니다.</p></div>
    {/if}
  </div>
</div>

{#snippet serviceRow(svc: Service, depth: number)}
  {@const st = app.statuses[svc.id]}
  {@const active = isActive(st?.state)}
  <div
    class="row service"
    class:selected={isSelected('service', svc.id)}
    class:drop-before={dropTarget?.id === svc.id}
    class:dragging={drag?.id === svc.id}
    style="--depth:{depth}"
    role="treeitem"
    aria-selected={isSelected('service', svc.id)}
    tabindex="-1"
    draggable={app.renaming !== svc.id}
    ondragstart={(e) => onDragStart(e, 'service', svc.id)}
    ondragover={(e) => onDragOver(e, 'service', svc.id)}
    ondrop={(e) => onDrop(e, 'service', svc)}
    ondragend={onDragEnd}
    onclick={() => app.select({ kind: 'service', id: svc.id })}
    ondblclick={() => !active && app.start(svc.id)}
    oncontextmenu={(e) => serviceMenu(e, svc)}
    onkeydown={() => {}}
  >
    <StatusDot state={st?.state ?? 'stopped'} />
    {#if app.renaming === svc.id}
      <input class="rename" value={svc.name} use:focusSelect onkeydown={(e) => renameKey(e, svc.id)} onblur={(e) => app.rename(svc.id, e.currentTarget.value)} onclick={(e) => e.stopPropagation()} />
    {:else}
      <span class="name" title={svc.name}>{svc.name}</span>
      {@const port = (active && st?.ports?.[0]) || svc.ports?.[0]}
      {#if st?.state === 'failed' && st.exitCode !== null}
        <span class="meta err">exit {st.exitCode}</span>
      {:else if st?.state === 'starting'}
        <span class="meta warn">시작 중</span>
      {:else if st?.state === 'waiting'}
        <span class="meta wait">대기 중</span>
      {:else if port}
        <span class="meta" class:live={active}>:{port}</span>
      {/if}
      <span class="actions">
        {#if active}
          <button class="act stop" title="중지 (Ctrl+F2)" onclick={(e) => { e.stopPropagation(); app.stop(svc.id) }}><Icon name="stop" size={14} /></button>
        {:else}
          <button class="act play" title="시작 (Shift+F10)" onclick={(e) => { e.stopPropagation(); app.start(svc.id) }}><Icon name="play" size={14} /></button>
        {/if}
      </span>
    {/if}
  </div>
{/snippet}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .tree-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--panel);
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    overflow: hidden;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
  }
  .tool {
    display: grid;
    place-items: center;
    width: 28px;
    height: 26px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .tool:hover:not(:disabled),
  .tool.active {
    background: var(--hover);
    color: var(--text);
  }
  .tool.stop:not(:disabled) {
    color: var(--err);
  }
  .tool:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .sep {
    width: 1px;
    height: 16px;
    margin: 0 4px;
    background: var(--border-strong);
  }
  .spacer {
    flex: 1;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 8px 4px;
    padding: 0 8px;
    height: 28px;
    border: 1px solid var(--input-border);
    border-radius: 6px;
    background: var(--input-bg);
    color: var(--muted);
  }
  .filter:focus-within {
    border-color: var(--accent);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    color: var(--text);
    font: inherit;
  }
  .clear {
    display: grid;
    place-items: center;
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
    padding: 2px;
  }
  .list {
    flex: 1;
    overflow: auto;
    padding: 4px 0 24px;
    outline: none;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    margin: 0 6px;
    padding: 0 6px 0 calc(8px + var(--depth, 0) * 22px);
    border-radius: 5px;
    color: var(--text);
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--sel-inactive);
  }
  .list:focus-within .row.selected {
    background: var(--sel);
  }
  .row.group {
    gap: 4px;
    padding-left: 2px;
  }
  .row.drop-before::before {
    content: '';
    position: absolute;
    left: 6px;
    right: 6px;
    top: -1px;
    height: 2px;
    border-radius: 1px;
    background: var(--accent);
  }
  .row.drop-into {
    outline: 1.5px dashed var(--accent);
    outline-offset: -1.5px;
  }
  .row.dragging {
    opacity: 0.5;
  }
  .twisty {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .folder {
    color: var(--folder);
    display: flex;
    margin-right: 3px;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count,
  .meta {
    color: var(--faint);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .count.live,
  .meta.live {
    color: var(--ok);
  }
  .meta.err {
    color: var(--err);
  }
  .meta.warn {
    color: var(--warn);
  }
  .meta.wait {
    color: var(--accent-text);
  }
  .actions {
    display: none;
    gap: 2px;
  }
  .row:hover .actions {
    display: flex;
  }
  .row:hover .meta,
  .row:hover .count {
    display: none;
  }
  .act {
    display: grid;
    place-items: center;
    width: 22px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }
  .act:hover {
    background: var(--hover-strong);
  }
  .act.play {
    color: var(--ok);
  }
  .act.stop {
    color: var(--err);
  }
  .rename {
    flex: 1;
    min-width: 0;
    height: 22px;
    padding: 0 6px;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
    outline: 0;
  }
  .section-head {
    margin: 2px 14px 2px;
    color: var(--faint);
    font-size: 11px;
    font-weight: 600;
  }
  .row.preset {
    gap: 6px;
    padding-left: 8px;
  }
  .pico {
    display: flex;
    color: var(--accent-text);
  }
  .divider {
    height: 1px;
    margin: 6px 12px;
    background: var(--border-strong);
  }
  .empty-group {
    margin: 0 6px;
    padding: 3px 8px 5px 52px;
    color: var(--faint);
    font-size: 12px;
  }
  .hint {
    padding: 24px 16px;
    color: var(--muted);
    text-align: center;
  }
  .hint p {
    margin: 0 0 12px;
  }
  .hint .secondary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    margin-top: 6px;
    padding: 0 12px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: none;
    color: var(--text);
    cursor: pointer;
  }
</style>
