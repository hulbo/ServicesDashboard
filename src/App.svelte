<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { app } from './lib/state.svelte'
  import Icon from './lib/components/Icon.svelte'
  import ServiceTree from './lib/components/ServiceTree.svelte'
  import ServiceDetail from './lib/components/ServiceDetail.svelte'
  import GroupDetail from './lib/components/GroupDetail.svelte'
  import WorkspaceSettings from './lib/components/WorkspaceSettings.svelte'
  import ImportPanel from './lib/components/ImportPanel.svelte'
  import RuntimesPanel from './lib/components/RuntimesPanel.svelte'
  import SettingsPanel from './lib/components/SettingsPanel.svelte'
  import PortConflictDialog from './lib/components/PortConflictDialog.svelte'
  import PresetPanel from './lib/components/PresetPanel.svelte'
  import BranchPanel from './lib/components/BranchPanel.svelte'
  import { headLabel } from './lib/git'
  import { applyFonts } from './lib/fonts'
  import Toasts from './lib/components/Toasts.svelte'
  import SidebarRail from './lib/components/SidebarRail.svelte'

  let initError = $state<string | null>(null)

  // 설정의 글꼴(미리보기 포함)을 문서 전체에 적용
  $effect(() => applyFonts(app.fonts))

  onMount(() => {
    app.init().catch((e) => (initError = String(e)))
  })

  // ── 사이드바 자동 숨김: 기둥이나 펼친 목록 위에 마우스가 있으면 펼치고, 벗어나면 잠시 뒤 접는다 ──
  let sidebarEl: HTMLElement | undefined = $state()
  let overRail = false
  let overSidebar = false
  let showTimer: ReturnType<typeof setTimeout> | undefined
  let hideTimer: ReturnType<typeof setTimeout> | undefined
  const OPEN_DELAY = 150
  const CLOSE_DELAY = 350

  /** 검색·이름 바꾸기처럼 사이드바 안에서 입력 중이면 마우스가 벗어나도 접지 않는다 */
  const typingInSidebar = () => {
    const el = document.activeElement as HTMLElement | null
    return !!el && !!sidebarEl?.contains(el) && !!el.closest('input, textarea, select')
  }

  function scheduleClose() {
    // 기둥 위에 있는 동안은 펼치려던 것을 취소하지 않는다
    if (!overRail) clearTimeout(showTimer)
    clearTimeout(hideTimer)
    hideTimer = setTimeout(() => {
      if (!overRail && !overSidebar && !typingInSidebar()) app.sidebarOpen = false
    }, CLOSE_DELAY)
  }

  function railEnter() {
    overRail = true
    clearTimeout(hideTimer)
    if (!app.sidebarOpen) {
      clearTimeout(showTimer)
      showTimer = setTimeout(() => {
        showTimer = undefined
        if (overRail) app.sidebarOpen = true
      }, OPEN_DELAY)
    }
  }
  /** mouseenter를 놓쳤거나 펼치기가 취소됐어도, 기둥 위에서 마우스를 움직이면 펼친다 */
  function railMove() {
    if (!overRail || (!app.sidebarOpen && showTimer === undefined)) railEnter()
  }
  function railLeave() {
    overRail = false
    clearTimeout(showTimer)
    showTimer = undefined
    scheduleClose()
  }
  function sidebarEnter() {
    overSidebar = true
    clearTimeout(hideTimer)
  }
  function sidebarLeave() {
    overSidebar = false
    if (!app.sidebarPinned) scheduleClose()
  }
  function sidebarFocusOut(e: FocusEvent) {
    // 입력칸에서 빠져나갔고 마우스도 밖에 있으면 접는다 (버튼 등에서 포커스가 빠지는 것은 상관없다)
    if (!app.sidebarPinned && (e.target as HTMLElement | null)?.closest?.('input, textarea, select')) scheduleClose()
  }
  function openSidebar() {
    clearTimeout(hideTimer)
    app.sidebarOpen = true
  }

  function onKeydown(e: KeyboardEvent) {
    const svc = app.selectedService
    if (e.key === 'F10' && e.shiftKey && svc) {
      e.preventDefault()
      void app.start(svc.id)
    } else if (e.key === 'F9' && e.shiftKey && svc) {
      e.preventDefault()
      void app.start(svc.id, true)
    } else if (e.key === 'F2' && e.ctrlKey && svc) {
      e.preventDefault()
      void app.stop(svc.id)
    } else if (e.key === 'F5' && e.ctrlKey && svc) {
      e.preventDefault()
      void app.restart(svc.id)
    } else if ((e.key === 'F5' || (e.ctrlKey && (e.key === 'r' || e.key === 'R'))) && app.ready && !app.backend.isMock) {
      // 실수로 화면을 새로 고치지 않게 막는다 (서비스는 계속 실행되지만 화면 상태가 초기화된다).
      e.preventDefault()
    } else if (e.ctrlKey && !e.shiftKey && !e.altKey && (e.key === 'b' || e.key === 'B')) {
      // 사이드바 고정 ↔ 자동 숨김
      e.preventDefault()
      app.toggleSidebarPinned()
    } else if (e.key === 'Escape' && !app.sidebarPinned && app.sidebarOpen && !(e.target as HTMLElement | null)?.closest?.('input, textarea, select, .menu')) {
      app.sidebarOpen = false
    } else if (e.ctrlKey && (e.key === 'k' || e.key === 'K')) {
      e.preventDefault()
      if (!app.sidebarPinned) openSidebar()
      void tick().then(() => document.querySelector<HTMLInputElement>('.filter input')?.focus())
    } else if (
      e.ctrlKey &&
      !e.altKey &&
      (e.key === 'a' || e.key === 'A') &&
      !(e.target as HTMLElement | null)?.closest?.('input, textarea, select, [contenteditable="true"], .console, .merged')
    ) {
      // 입력칸·콘솔 밖에서는 화면 전체(트리·버튼 글자)가 선택되지 않게 한다. 콘솔은 자기 내용만 선택한다.
      e.preventDefault()
    }
  }

  function onContextMenu(e: MouseEvent) {
    // 앱 안에서는 브라우저 기본 메뉴(뒤로, 새로 고침 등)를 숨긴다. 입력칸과 콘솔은 복사·붙여넣기를 위해 둔다.
    if (!app.ready || app.backend.isMock) return
    const el = e.target as HTMLElement
    if (el.closest('input, textarea, .viewport, code')) return
    e.preventDefault()
  }

  function startResize(e: PointerEvent) {
    const target = e.currentTarget as HTMLElement
    target.setPointerCapture(e.pointerId)
    const move = (ev: PointerEvent) => app.setSidebarWidth(ev.clientX)
    const up = () => {
      target.removeEventListener('pointermove', move)
      target.removeEventListener('pointerup', up)
    }
    target.addEventListener('pointermove', move)
    target.addEventListener('pointerup', up)
  }
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={onContextMenu} />

{#if initError}
  <div class="center">
    <h2>앱을 시작하지 못했습니다</h2>
    <p class="mono">{initError}</p>
  </div>
{:else if !app.ready}
  <div class="center muted">불러오는 중…</div>
{:else if app.loadError}
  <div class="center">
    <h2>워크스페이스 파일을 읽지 못했습니다</h2>
    <p class="mono err">{app.loadError}</p>
    <p class="muted">파일을 고친 뒤 다시 불러오세요. 이 상태에서는 파일을 덮어쓰지 않습니다.</p>
    <div class="row">
      {#if app.info}
        <button class="btn" onclick={() => app.info && app.backend.openPath(app.info.configDir)}><Icon name="folder" size={14} /> 설정 폴더 열기</button>
      {/if}
      <button class="btn primary" onclick={() => app.loadWorkspace(true)}><Icon name="refresh" size={14} /> 다시 불러오기</button>
    </div>
  </div>
{:else}
  <div class="app" class:autohide={!app.sidebarPinned} style="--sidebar:{app.sidebarWidth}px">
    {#if !app.sidebarPinned}
      <SidebarRail onenter={railEnter} onleave={railLeave} onmove={railMove} onopen={openSidebar} />
    {/if}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <aside
      class="sidebar"
      class:open={app.sidebarOpen}
      bind:this={sidebarEl}
      onmouseenter={sidebarEnter}
      onmouseleave={sidebarLeave}
      onfocusout={sidebarFocusOut}
    >
      <ServiceTree />
    </aside>
    {#if app.sidebarPinned}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="splitter" onpointerdown={startResize} ondblclick={() => app.setSidebarWidth(300)}></div>
    {/if}
    <main class="main">
      {#if app.selectedService}
        {#key app.selectedService.id}
          <ServiceDetail service={app.selectedService} />
        {/key}
      {:else if app.selectedGroup}
        {#key app.selectedGroup.id}
          <GroupDetail group={app.selectedGroup} />
        {/key}
      {:else if app.selectedPreset}
        {#key app.selectedPreset.id}
          <PresetPanel preset={app.selectedPreset} />
        {/key}
      {:else if app.selection?.kind === 'workspace'}
        <WorkspaceSettings />
      {:else if app.selection?.kind === 'import'}
        <ImportPanel />
      {:else if app.selection?.kind === 'runtimes'}
        <RuntimesPanel />
      {:else if app.selection?.kind === 'settings'}
        <SettingsPanel />
      {:else if app.selection?.kind === 'branches'}
        <BranchPanel />
      {:else}
        <div class="center">
          <h2>Services Dashboard</h2>
          <p class="muted">왼쪽에서 서비스를 선택하거나 새로 추가하세요.</p>
          <div class="row">
            <button class="btn primary" onclick={() => app.select({ kind: 'import' })}><Icon name="import" size={14} /> 프로젝트에서 가져오기</button>
            <button class="btn" onclick={() => app.addService()}><Icon name="plus" size={14} /> 서비스 추가</button>
            <button class="btn" onclick={() => app.addGroup()}><Icon name="folder-plus" size={14} /> 그룹 추가</button>
            <button class="btn" onclick={() => app.select({ kind: 'workspace' })}><Icon name="workspace" size={14} /> 워크스페이스 설정</button>
          </div>
          <dl class="keys">
            <dt>Shift+F10</dt><dd>시작</dd>
            <dt>Ctrl+F2</dt><dd>중지</dd>
            <dt>Ctrl+F5</dt><dd>재시작</dd>
            <dt>Shift+F9</dt><dd>디버그로 시작</dd>
            <dt>Ctrl+K</dt><dd>서비스 검색</dd>
            <dt>Ctrl+B</dt><dd>사이드바 고정 / 자동 숨김</dd>
            <dt>F2</dt><dd>이름 바꾸기</dd>
          </dl>
        </div>
      {/if}
    </main>
    <footer class="statusbar">
      <span class="live" class:on={app.runningCount > 0}>●</span>
      <span>실행 {app.runningCount} / 전체 {app.workspace.services.length}</span>
      <span class="sep">·</span>
      <button class="ws" title={app.workspacePath} onclick={() => app.select({ kind: 'workspace' })}>{app.workspace.name || '워크스페이스'}</button>
      {#each app.repos.slice(0, 4) as r (r.root)}
        <span class="sep">·</span>
        <button class="ws branch" title="{r.root} — 브랜치 보기·전환" onclick={() => app.openBranches(r.root)}>
          <Icon name="branch" size={11} />
          {app.repos.length > 1 ? `${r.name}: ` : ''}{headLabel(r)}
        </button>
      {/each}
      {#if app.repos.length > 4}<button class="ws" onclick={() => app.openBranches()}>외 {app.repos.length - 4}개</button>{/if}
      <span class="spacer"></span>
      {#if app.backend.isMock}
        <span class="badge" title="일반 브라우저에서 열어 가짜 백엔드로 동작 중입니다. 실제 프로세스는 실행되지 않습니다.">브라우저 미리보기</span>
      {/if}
      {#if app.info}<span class="ver">v{app.info.version}</span>{/if}
    </footer>
  </div>
{/if}

<Toasts />
<PortConflictDialog />

<style>
  .app {
    position: relative;
    display: grid;
    grid-template-columns: var(--sidebar) 4px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr) 24px;
    height: 100vh;
  }
  .sidebar {
    grid-row: 1;
    min-width: 0;
    border-right: 1px solid var(--border);
  }
  /* 자동 숨김: 왼쪽 기둥(44px) + 본문. 목록은 기둥 오른쪽에 겹쳐서 펼쳐진다 */
  .app.autohide {
    --rail: 44px;
    grid-template-columns: var(--rail) minmax(0, 1fr);
  }
  /* 열을 직접 정한다: .main은 grid-row가 정해져 있어 자동 배치 때 기둥보다 먼저 1열을 차지한다 */
  .app.autohide :global(.rail) {
    grid-row: 1;
    grid-column: 1;
  }
  .app.autohide .main {
    grid-column: 2;
  }
  .app.autohide .sidebar {
    position: absolute;
    top: 0;
    bottom: 24px;
    left: var(--rail);
    z-index: 40;
    width: var(--sidebar);
    background: var(--panel);
    box-shadow: 6px 0 18px rgb(0 0 0 / 0.35);
    transform: translateX(calc(-100% - var(--rail)));
    visibility: hidden;
    transition:
      transform 0.16s ease,
      visibility 0s linear 0.16s;
  }
  .app.autohide .sidebar.open {
    transform: none;
    visibility: visible;
    transition: transform 0.16s ease;
  }
  @media (prefers-reduced-motion: reduce) {
    .app.autohide .sidebar,
    .app.autohide .sidebar.open {
      transition: none;
    }
  }
  .splitter {
    grid-row: 1;
    cursor: col-resize;
    background: var(--bg);
  }
  .splitter:hover {
    background: var(--accent);
  }
  .main {
    grid-row: 1;
    min-width: 0;
    min-height: 0;
    background: var(--bg);
  }
  .statusbar {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    border-top: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted);
    font-size: 11.5px;
  }
  .live {
    color: var(--idle);
    font-size: 9px;
  }
  .live.on {
    color: var(--ok);
  }
  .sep {
    color: var(--faint);
  }
  .ws {
    padding: 0 4px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .ws:hover {
    color: var(--text);
  }
  .ws.branch {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  .badge {
    padding: 0 6px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--warn) 18%, transparent);
    color: var(--warn);
  }
  .ver {
    color: var(--faint);
  }
  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 100%;
    min-height: 100vh;
    padding: 24px;
    text-align: center;
  }
  .main .center {
    min-height: 0;
  }
  .center h2 {
    margin: 0 0 4px;
    font-size: 18px;
  }
  .muted {
    color: var(--muted);
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
    max-width: 720px;
    word-break: break-all;
    user-select: text;
  }
  .err {
    color: var(--err);
  }
  .row {
    display: flex;
    gap: 8px;
    margin-top: 12px;
    flex-wrap: wrap;
    justify-content: center;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 14px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--panel);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .btn:hover {
    background: var(--hover);
  }
  .btn.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
  .keys {
    display: grid;
    grid-template-columns: auto auto;
    gap: 4px 14px;
    margin: 28px 0 0;
    color: var(--faint);
    font-size: 12px;
  }
  .keys dt {
    text-align: right;
    font-family: var(--font-mono);
    color: var(--muted);
  }
  .keys dd {
    margin: 0;
    text-align: left;
  }
</style>
