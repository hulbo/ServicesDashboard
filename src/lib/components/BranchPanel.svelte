<script lang="ts">
  import { untrack } from 'svelte'
  import { app } from '../state.svelte'
  import { errorMessage } from '../api'
  import { headLabel, matchBranch } from '../git'
  import type { BranchInfo, GitRepo } from '../types'
  import { isActive } from '../types'
  import Icon from './Icon.svelte'
  import StatusDot from './StatusDot.svelte'

  /** 한 번에 보여 줄 브랜치 수 (나머지는 검색으로 좁힌다) */
  const LIMIT = 80

  let info = $state<BranchInfo | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let query = $state('')
  let pull = $state(false)
  let restart = $state(true)
  let fetching = $state(false)
  /** 전환 중인 브랜치 */
  let busy = $state<string | null>(null)

  const focused = $derived(app.repos.find((r) => r.root === app.branchFocus))
  const focusedHead = $derived(focused ? headLabel(focused) : '')
  const locals = $derived(info ? info.locals.filter((b) => matchBranch(b.name, query)) : [])
  const remotes = $derived(info ? info.remotes.filter((b) => matchBranch(b, query)) : [])

  // 펼친 저장소가 바뀌거나 밖에서 브랜치가 바뀌면 목록을 다시 읽는다
  $effect(() => {
    const root = focused?.root
    void focusedHead
    untrack(() => {
      if (root) void load(root)
      else info = null
    })
  })

  async function load(root: string) {
    loading = true
    error = null
    try {
      const r = await app.backend.gitBranches(root)
      if (root === app.branchFocus) info = r
    } catch (e) {
      if (root === app.branchFocus) {
        info = null
        error = errorMessage(e)
      }
    } finally {
      loading = false
    }
  }

  function toggle(root: string) {
    app.branchFocus = app.branchFocus === root ? null : root
    query = ''
  }

  function running(repo: GitRepo): string[] {
    return repo.services.filter((id) => isActive(app.statuses[id]?.state))
  }

  async function doFetch(repo: GitRepo) {
    fetching = true
    try {
      await app.backend.gitFetch(repo.root)
      app.toast('success', `${repo.name}: 원격 브랜치를 가져왔습니다.`)
    } catch (e) {
      app.toast('error', `원격 가져오기 실패: ${errorMessage(e)}`)
    } finally {
      fetching = false
    }
    await load(repo.root)
  }

  async function doSwitch(repo: GitRepo, branch: string) {
    busy = branch
    await app.switchBranch(repo, branch, pull, restart)
    busy = null
    await load(repo.root)
  }
</script>

<div class="panel">
  <header class="head">
    <Icon name="branch" size={18} />
    <h2>브랜치</h2>
    <span class="sub">서비스가 쓰는 git 저장소</span>
    <span class="spacer"></span>
    <button class="btn" disabled={loading} onclick={() => { void app.refreshRepos(); if (focused) void load(focused.root) }}>
      <Icon name="refresh" size={14} /> 새로 고침
    </button>
  </header>

  <div class="scroll">
    <p class="intro">
      브랜치를 고르면 그 저장소에서 실행 중인 서비스를 멈추고 전환한 뒤 다시 시작합니다. 커밋하지 않은 변경이 있으면 전환하지 않습니다(자동 stash 없음).
      대시보드는 커밋·push를 하지 않습니다. IntelliJ나 터미널에서 브랜치를 바꿔도 몇 초 안에 알려 줍니다.
    </p>

    {#if app.repos.length === 0}
      <p class="empty">git 저장소 안에서 실행하는 서비스가 없습니다. 서비스의 작업 디렉터리가 git 저장소 안에 있으면 여기에 나타납니다.</p>
    {/if}

    {#each app.repos as repo (repo.root)}
      {@const open = app.branchFocus === repo.root}
      {@const run = running(repo)}
      <section class="repo" class:open>
        <button class="repo-head" onclick={() => toggle(repo.root)} aria-expanded={open}>
          <Icon name={open ? 'chevron-down' : 'chevron-right'} size={14} />
          <b class="name">{repo.name}</b>
          <span class="chip" class:detached={!repo.branch}><Icon name="branch" size={12} /> {headLabel(repo)}</span>
          {#if app.switching[repo.root]}<span class="muted">전환 중…</span>{/if}
          <span class="muted">서비스 {repo.services.length}개{run.length ? ` · 실행 중 ${run.length}` : ''}</span>
          <span class="root mono" title={repo.root}>{repo.root}</span>
        </button>

        {#if open}
          <div class="body">
            <div class="svcs">
              {#each repo.services as id (id)}
                <button class="svc" onclick={() => app.select({ kind: 'service', id })}>
                  <StatusDot state={app.statuses[id]?.state ?? 'stopped'} size={7} />
                  {app.serviceName(id)}
                </button>
              {/each}
            </div>

            {#if error}
              <p class="error">{error}</p>
            {:else if !info}
              <p class="empty">불러오는 중…</p>
            {:else}
              <div class="status">
                {#if info.changes.length}
                  <details class="dirty">
                    <summary><Icon name="warning" size={12} /> 커밋하지 않은 변경 {info.changes.length}개 — 전환하려면 먼저 커밋하거나 stash하세요</summary>
                    <ul>
                      {#each info.changes as c, i (i)}<li class="mono">{c}</li>{/each}
                    </ul>
                  </details>
                {:else}
                  <span class="clean">커밋하지 않은 변경 없음</span>
                {/if}
                {#if info.ahead !== null && info.behind !== null}
                  <span class="sync" title="원격 추적 브랜치보다 앞선(↑)·뒤처진(↓) 커밋 수. 원격 가져오기 후 정확해집니다.">↑{info.ahead} ↓{info.behind}</span>
                {/if}
              </div>

              <div class="opts">
                <label><input type="checkbox" bind:checked={pull} /> 전환 후 pull (fast-forward만)</label>
                <label><input type="checkbox" bind:checked={restart} /> 실행 중인 서비스 다시 시작{run.length ? ` (${run.length}개)` : ''}</label>
                <span class="spacer"></span>
                <button class="btn" disabled={fetching} onclick={() => doFetch(repo)}>
                  <Icon name="refresh" size={13} /> {fetching ? '가져오는 중…' : '원격 가져오기 (fetch)'}
                </button>
              </div>

              <label class="filter">
                <Icon name="search" size={14} />
                <input placeholder="브랜치 검색" bind:value={query} spellcheck="false" />
              </label>

              <h4>로컬 브랜치 <span class="n">{locals.length}</span></h4>
              <ul class="branches">
                {#each locals.slice(0, LIMIT) as b (b.name)}
                  {@const cur = b.name === info.head.branch}
                  <li class:cur>
                    <span class="bname mono" title={b.name}>{b.name}</span>
                    {#if b.upstream}<span class="up mono">→ {b.upstream}</span>{/if}
                    <span class="spacer"></span>
                    {#if cur}
                      <span class="current">현재</span>
                    {:else}
                      <button class="btn small" disabled={!!busy || !!app.switching[repo.root]} onclick={() => doSwitch(repo, b.name)}>
                        {busy === b.name ? '전환 중…' : '전환'}
                      </button>
                    {/if}
                  </li>
                {/each}
                {#if locals.length > LIMIT}<li class="more">외 {locals.length - LIMIT}개 — 검색으로 좁히세요</li>{/if}
                {#if locals.length === 0}<li class="more">맞는 브랜치가 없습니다.</li>{/if}
              </ul>

              {#if info.remotes.length}
                <h4>원격 브랜치 <span class="n">{remotes.length}</span> <span class="hint">고르면 같은 이름의 로컬 브랜치를 만들어 추적합니다</span></h4>
                <ul class="branches">
                  {#each remotes.slice(0, LIMIT) as b (b)}
                    <li>
                      <span class="bname mono" title={b}>{b}</span>
                      <span class="spacer"></span>
                      <button class="btn small" disabled={!!busy || !!app.switching[repo.root]} onclick={() => doSwitch(repo, b)}>
                        {busy === b ? '전환 중…' : '가져와서 전환'}
                      </button>
                    </li>
                  {/each}
                  {#if remotes.length > LIMIT}<li class="more">외 {remotes.length - LIMIT}개 — 검색으로 좁히세요</li>{/if}
                </ul>
              {/if}
            {/if}
          </div>
        {/if}
      </section>
    {/each}
  </div>
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
    color: var(--faint);
    font-size: 12px;
  }
  .spacer {
    flex: 1;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px 20px 32px;
  }
  .intro {
    max-width: 880px;
    margin: 0 0 16px;
    color: var(--muted);
    font-size: 12.5px;
    line-height: 1.6;
  }
  .empty,
  .more {
    color: var(--faint);
    font-size: 12px;
  }
  .error {
    color: var(--err);
    font-size: 12.5px;
    white-space: pre-wrap;
  }
  .muted {
    color: var(--faint);
    font-size: 12px;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 11.5px;
  }
  .repo {
    max-width: 980px;
    margin-bottom: 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
  }
  .repo.open {
    border-color: var(--border-strong);
  }
  .repo-head {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 10px 12px;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .repo-head:hover {
    background: var(--hover);
  }
  .name {
    font-size: 13.5px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 8px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--text);
    font-size: 12px;
  }
  .chip.detached {
    background: color-mix(in srgb, var(--warn) 22%, transparent);
  }
  .root {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
    color: var(--faint);
  }
  .body {
    padding: 4px 14px 14px 36px;
  }
  .svcs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }
  .svc {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel-2);
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .svc:hover {
    color: var(--text);
  }
  .status {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 10px;
    font-size: 12.5px;
  }
  .clean {
    color: var(--ok);
  }
  .dirty {
    flex: 1;
    color: var(--warn);
  }
  .dirty summary {
    display: flex;
    align-items: center;
    gap: 5px;
    cursor: pointer;
  }
  .dirty ul {
    max-height: 160px;
    overflow: auto;
    margin: 6px 0 0;
    padding-left: 18px;
    color: var(--muted);
  }
  .sync {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
    margin-bottom: 10px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .opts label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 420px;
    padding: 0 8px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--panel-2);
    color: var(--faint);
  }
  .filter input {
    flex: 1;
    height: 28px;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font: inherit;
  }
  h4 {
    margin: 14px 0 6px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .n {
    color: var(--faint);
    font-weight: 400;
  }
  .hint {
    margin-left: 6px;
    color: var(--faint);
    font-weight: 400;
    font-size: 11.5px;
  }
  .branches {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .branches li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 30px;
    padding: 2px 8px;
    border-bottom: 1px solid var(--border);
  }
  .branches li:hover {
    background: var(--hover);
  }
  .branches li.cur .bname {
    color: var(--accent);
    font-weight: 600;
  }
  .bname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .up {
    color: var(--faint);
    white-space: nowrap;
  }
  .current {
    color: var(--accent);
    font-size: 12px;
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
  .btn.small {
    height: 24px;
    padding: 0 10px;
    font-size: 12px;
  }
  .btn:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
