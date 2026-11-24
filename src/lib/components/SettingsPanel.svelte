<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { CONSOLE_SIZES, DEFAULT_FONTS, cssFamily, fontLabel, fontsOf } from '../fonts'
  import { app } from '../state.svelte'
  import type { AppSettings, FontSettings } from '../types'
  import { progressPercent, releaseUrl } from '../update'
  import Icon from './Icon.svelte'

  const cloneSettings = (s: AppSettings | null): AppSettings | null => {
    if (!s) return null
    const c = structuredClone($state.snapshot(s) as AppSettings)
    c.fonts = fontsOf(c)
    return c
  }

  let draft = $state<AppSettings | null>(cloneSettings(app.settings))

  $effect(() => {
    if (!draft && app.settings) draft = cloneSettings(app.settings)
  })

  // 글꼴은 고르는 즉시 앱 전체에 미리 보여 주고, 저장하지 않고 떠나면 저장된 글꼴로 돌아간다.
  $effect(() => {
    app.fontPreview = draft ? ($state.snapshot(draft.fonts) as FontSettings) : null
  })
  onDestroy(() => (app.fontPreview = null))
  onMount(() => void app.loadSystemFonts())

  const fonts = $derived(app.systemFonts ?? [])
  const monoFonts = $derived(fonts.filter((f) => f.mono))
  const otherFonts = $derived(fonts.filter((f) => !f.mono))
  /** 저장된 글꼴이 지금 PC에 없으면 목록에 따로 보여 준다 */
  const missing = (name: string) => !!name && !!app.systemFonts && !fonts.some((f) => f.name.toLowerCase() === name.toLowerCase())
  const sizes = $derived(draft ? [...new Set([...CONSOLE_SIZES, draft.fonts.consoleSize])].sort((a, b) => a - b) : CONSOLE_SIZES)
  const isDefaultFonts = $derived(!!draft && JSON.stringify(draft.fonts) === JSON.stringify(DEFAULT_FONTS))

  const dirty = $derived(!!draft && !!app.settings && JSON.stringify(draft) !== JSON.stringify(app.settings))

  async function save() {
    if (!draft) return
    const next = $state.snapshot(draft) as AppSettings
    next.logFiles.retentionDays = Math.max(0, Math.min(3650, Math.round(Number(next.logFiles.retentionDays) || 0)))
    if (await app.saveSettings(next)) app.toast('success', '설정을 저장했습니다.')
  }

  const upd = $derived(app.update)
  const updBusy = $derived(upd.phase === 'checking' || upd.phase === 'downloading' || upd.phase === 'installing')
  const updStatus = $derived.by(() => {
    switch (upd.phase) {
      case 'checking':
        return '확인 중…'
      case 'latest':
        return '최신 버전입니다.'
      case 'downloading': {
        const pct = progressPercent(upd.progress)
        return pct === null ? '받는 중…' : `받는 중 ${pct}%`
      }
      case 'installing':
        return '설치 준비 중…'
      case 'error':
        return upd.error || '업데이트를 확인하지 못했습니다.'
      default:
        return ''
    }
  })

  async function openLogs() {
    if (!app.info) return
    await app.backend.openPath(`${app.info.configDir}\\logs`).catch(() => app.backend.openPath(app.info!.configDir))
  }
</script>

<div class="panel">
  <header class="head">
    <Icon name="gear" size={18} />
    <h2>설정</h2>
    <span class="spacer"></span>
    <button class="btn" disabled={!dirty} onclick={() => (draft = cloneSettings(app.settings))}>되돌리기</button>
    <button class="btn primary" disabled={!dirty} onclick={save}>저장</button>
  </header>

  {#if draft}
    <div class="scroll">
      <section class="update">
        <h3>업데이트</h3>
        <div class="row">
          <span>현재 버전 <b>v{app.info?.version ?? '?'}</b></span>
          <button class="btn" disabled={updBusy} onclick={() => app.checkUpdate(true)}><Icon name="refresh" size={14} /> 지금 확인</button>
          {#if updStatus}<span class="upd-status" class:err={upd.phase === 'error'}>{updStatus}</span>{/if}
        </div>
        {#if upd.phase === 'available' && upd.info}
          {@const info = upd.info}
          <div class="upd-new">
            <div><b>v{info.version}</b>이 나왔습니다.</div>
            {#if info.notes}<pre class="notes">{info.notes}</pre>{/if}
            <div class="row">
              <button class="btn primary" onclick={() => app.installUpdate()}><Icon name="update" size={14} /> 지금 업데이트</button>
              <button class="btn" onclick={() => app.backend.openUrl(releaseUrl(info.version))}>릴리스 페이지</button>
            </div>
          </div>
        {/if}
        <label class="check auto">
          <input type="checkbox" bind:checked={draft.autoUpdateCheck} />
          앱을 켤 때 새 버전 확인
        </label>
        <p class="hint">
          새 버전이 있으면 아래 상태 표시줄에 <b>업데이트</b> 버튼이 나타납니다. 누르면 설치본을 받아 서명을 확인한 뒤 설치하고, 앱이 잠시
          꺼졌다가 새 버전으로 다시 켜집니다. 실행 중인 서비스는 먼저 중지합니다.
        </p>
      </section>

      <section>
        <h3>창</h3>
        <label class="check">
          <input type="checkbox" bind:checked={draft.closeToTray} />
          창을 닫으면 트레이로 내리기
        </label>
        <p class="hint">켜 두면 창을 닫아도 서비스가 계속 실행됩니다. 트레이 아이콘을 누르면 다시 열리고, 트레이 메뉴에서 그룹별로 시작·중지하거나 앱을 종료할 수 있습니다. 끄면 창을 닫을 때 앱과 서비스가 함께 종료됩니다.</p>
      </section>

      <section>
        <h3>알림</h3>
        <label class="check">
          <input type="checkbox" bind:checked={draft.notifications} />
          Windows 알림 보내기
        </label>
        <p class="hint">서비스가 비정상 종료되면 알립니다. 준비 완료 알림은 대시보드 창을 보고 있지 않을 때만 보냅니다.</p>
      </section>

      <section>
        <h3>글꼴</h3>
        <div class="font-row">
          <label for="s-font-ui">화면 글꼴</label>
          <select id="s-font-ui" bind:value={draft.fonts.ui}>
            <option value="">기본 (Pretendard → 맑은 고딕)</option>
            {#if missing(draft.fonts.ui)}<option value={draft.fonts.ui}>{draft.fonts.ui} (설치되지 않음)</option>{/if}
            {#each fonts as f (f.name)}
              <option value={f.name} style:font-family={cssFamily(f.name)}>{fontLabel(f)}</option>
            {/each}
          </select>
        </div>
        <div class="font-row">
          <label for="s-font-mono">콘솔 글꼴</label>
          <select id="s-font-mono" bind:value={draft.fonts.mono}>
            <option value="">기본 (JetBrains Mono → D2Coding → Consolas)</option>
            {#if missing(draft.fonts.mono)}<option value={draft.fonts.mono}>{draft.fonts.mono} (설치되지 않음)</option>{/if}
            {#if monoFonts.length}
              <optgroup label="고정폭 글꼴 (콘솔에 알맞음)">
                {#each monoFonts as f (f.name)}
                  <option value={f.name} style:font-family={cssFamily(f.name)}>{fontLabel(f)}</option>
                {/each}
              </optgroup>
            {/if}
            {#if otherFonts.length}
              <optgroup label="그 밖의 글꼴">
                {#each otherFonts as f (f.name)}
                  <option value={f.name} style:font-family={cssFamily(f.name)}>{fontLabel(f)}</option>
                {/each}
              </optgroup>
            {/if}
          </select>
          <label for="s-font-size" class="size-label">크기</label>
          <select id="s-font-size" class="size" bind:value={draft.fonts.consoleSize}>
            {#each sizes as n (n)}
              <option value={n}>{n}{n === DEFAULT_FONTS.consoleSize ? ' (기본)' : ''}</option>
            {/each}
          </select>
          <span class="muted">px</span>
        </div>
        <div class="font-preview">
          <div class="ui-sample">서비스 대시보드 · 가나다라마바사 ABC abc 0123</div>
          <div class="mono-sample"><span class="ts">2026-11-20 10:42:01</span> <span class="lv">INFO</span> Started PortalApplication in 3.4 seconds · 한글 로그 0O1lI</div>
        </div>
        <p class="hint">
          고르면 앱 전체에 바로 미리 보이고, <b>저장</b>해야 유지됩니다. 고른 글꼴에 없는 글자(예: 한글)는 기본 글꼴로 보입니다.
          {#if app.systemFonts === null}설치된 글꼴을 읽는 중…{/if}
        </p>
        <button class="btn" disabled={isDefaultFonts} onclick={() => draft && (draft.fonts = { ...DEFAULT_FONTS })}>기본 글꼴로</button>
      </section>

      <section>
        <h3>콘솔</h3>
        <label class="check">
          <input type="checkbox" bind:checked={draft.clearConsoleOnStart} />
          시작할 때 이전 콘솔 로그 지우기
        </label>
        <p class="hint">
          서비스를 직접 시작·재시작하면(그룹·프리셋·브랜치 전환 포함) 이전 실행의 로그를 지우고 새로 보여 줍니다. 비정상 종료 후 <b>자동 재시작</b>할 때는 원인을 볼 수 있게 지우지 않습니다. 로그 파일에는 그대로 남습니다.
        </p>
      </section>

      <section>
        <h3>로그 파일</h3>
        <label class="check">
          <input type="checkbox" bind:checked={draft.logFiles.enabled} />
          콘솔 로그를 파일로도 저장
        </label>
        <div class="row">
          <label for="s-ret">보관 기간</label>
          <input id="s-ret" type="number" min="0" max="3650" bind:value={draft.logFiles.retentionDays} disabled={!draft.logFiles.enabled} />
          <span class="muted">일 (0이면 지우지 않음)</span>
        </div>
        <p class="hint">
          서비스별로 하루에 한 파일씩 <code>설정 폴더\logs\&lt;서비스 ID&gt;\YYYY-MM-DD.log</code>에 저장합니다. 색상 코드는 빼고, 수신 시각과 출력 종류(OUT·ERR·SYS)를 붙입니다.
        </p>
        <button class="btn" onclick={openLogs}><Icon name="folder" size={14} /> 로그 폴더 열기</button>
      </section>
    </div>
  {:else}
    <p class="loading">설정을 불러오는 중…</p>
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
  .spacer {
    flex: 1;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 4px 20px 24px;
  }
  section {
    max-width: 760px;
    padding: 14px 0;
    border-bottom: 1px solid var(--border);
  }
  section:last-child {
    border-bottom: 0;
  }
  h3 {
    margin: 0 0 10px;
    color: var(--muted);
    font-size: 11.5px;
    font-weight: 600;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .check input {
    accent-color: var(--accent);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
  .row input {
    width: 80px;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
  }
  .row input:disabled {
    opacity: 0.5;
  }
  .upd-status {
    color: var(--muted);
  }
  .upd-status.err {
    color: var(--err);
  }
  .upd-new {
    margin-top: 10px;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--border));
    border-radius: 6px;
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .upd-new .row {
    margin-top: 8px;
  }
  .notes {
    max-height: 9em;
    margin: 6px 0 0;
    overflow: auto;
    color: var(--muted);
    font-family: inherit;
    font-size: 12px;
    white-space: pre-wrap;
  }
  .check.auto {
    margin-top: 12px;
  }
  .font-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }
  .font-row label {
    flex: none;
    width: 72px;
    color: var(--muted);
  }
  .font-row .size-label {
    width: auto;
    margin-left: 8px;
  }
  select {
    flex: 1;
    min-width: 0;
    max-width: 380px;
    height: 28px;
    padding: 0 6px;
    border: 1px solid var(--input-border);
    border-radius: 5px;
    background: var(--input-bg);
    color: var(--text);
    font: inherit;
  }
  select.size {
    flex: none;
    width: 96px;
  }
  .font-preview {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 12px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--console-bg, var(--panel));
  }
  .ui-sample {
    font-family: var(--font-ui);
    font-size: 13px;
  }
  .mono-sample {
    font-family: var(--font-mono);
    font-size: var(--console-font-size);
    line-height: var(--console-row);
    color: var(--console-fg, var(--text));
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mono-sample .ts {
    color: var(--faint);
  }
  .mono-sample .lv {
    color: var(--ok);
  }
  .hint,
  .muted {
    color: var(--faint);
    font-size: 12px;
  }
  .hint {
    margin: 8px 0 10px;
    line-height: 1.6;
  }
  code {
    font-family: var(--font-mono);
    font-size: 11.5px;
  }
  .loading {
    padding: 20px;
    color: var(--muted);
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
</style>
