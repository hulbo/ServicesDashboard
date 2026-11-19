<script lang="ts">
  import { app } from '../state.svelte'
  import type { RuntimeKind } from '../types'
  import Icon from './Icon.svelte'

  const SECTIONS: { kind: RuntimeKind; title: string; hint: string }[] = [
    { kind: 'jdk', title: 'JDK', hint: 'JAVA_HOME, Program Files, IntelliJ가 받은 JDK(~/.jdks), IntelliJ JDK 목록, PATH에서 찾습니다.' },
    { kind: 'node', title: 'Node.js', hint: 'PATH, Program Files, nvm-windows, Volta, fnm에서 찾습니다.' },
    { kind: 'maven', title: 'Maven', hint: 'MAVEN_HOME, PATH, IntelliJ 번들 Maven에서 찾습니다. 프로젝트에 mvnw가 있으면 그것이 우선입니다.' },
  ]
</script>

<div class="panel">
  <header class="head">
    <Icon name="runtime" size={18} />
    <h2>런타임</h2>
    <span class="spacer"></span>
    <button class="btn" disabled={app.runtimesLoading} onclick={() => app.loadRuntimes(true)}>
      <Icon name="refresh" size={14} /> {app.runtimesLoading ? '찾는 중…' : '다시 감지'}
    </button>
  </header>

  <div class="scroll">
    <p class="intro">
      서비스 설정의 JDK·Node·Maven 항목에서 아래 설치를 고르거나 <b>버전 별칭</b>(예: <code>17</code>, <code>1.8</code>, <code>20</code>)을 쓸 수
      있습니다. 별칭은 실행할 때 이 PC에 설치된 같은 주 버전 중 가장 높은 버전으로 바뀌어, 다른 PC와 설정을 나눠 쓰기 좋습니다.
    </p>
    {#each SECTIONS as sec (sec.kind)}
      {@const list = app.runtimes.filter((r) => r.kind === sec.kind)}
      <section>
        <h3>{sec.title} <span class="n">{list.length}</span></h3>
        <p class="hint">{sec.hint}</p>
        {#if list.length === 0}
          <p class="empty">{app.runtimesLoading ? '찾는 중…' : '찾지 못했습니다.'}</p>
        {:else}
          <table>
            <thead><tr><th>버전</th><th>별칭</th><th>제공·출처</th><th>경로</th><th></th></tr></thead>
            <tbody>
              {#each list as r (r.path)}
                <tr>
                  <td class="ver">{r.version}</td>
                  <td><code>{r.major}</code></td>
                  <td>{r.vendor ? `${r.vendor} · ` : ''}{r.source}</td>
                  <td class="path" title={r.path}>{r.path}</td>
                  <td><button class="icon" title="폴더 열기" onclick={() => app.backend.openPath(r.path)}><Icon name="folder" size={14} /></button></td>
                </tr>
              {/each}
            </tbody>
          </table>
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
  .spacer {
    flex: 1;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 8px 20px 24px;
  }
  .intro {
    max-width: 860px;
    color: var(--muted);
    font-size: 12.5px;
    line-height: 1.6;
  }
  section {
    max-width: 980px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
  }
  h3 {
    margin: 0 0 4px;
    font-size: 13px;
  }
  .n {
    color: var(--faint);
    font-weight: 400;
  }
  .hint,
  .empty {
    margin: 0 0 8px;
    color: var(--faint);
    font-size: 12px;
  }
  code {
    font-family: var(--font-mono);
    font-size: 11.5px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  th {
    text-align: left;
    padding: 4px 8px;
    color: var(--faint);
    font-weight: 500;
    border-bottom: 1px solid var(--border-strong);
  }
  td {
    padding: 5px 8px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  .ver {
    font-variant-numeric: tabular-nums;
    color: var(--text);
  }
  .path {
    max-width: 420px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--muted);
  }
  .icon {
    display: grid;
    place-items: center;
    padding: 3px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .icon:hover {
    background: var(--hover);
    color: var(--text);
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
    opacity: 0.5;
  }
</style>
