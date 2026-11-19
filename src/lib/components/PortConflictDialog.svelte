<script lang="ts">
  import { app } from '../state.svelte'
  import Icon from './Icon.svelte'

  const c = $derived(app.conflict)
  const ownedByService = $derived(c?.owners.some((o) => o.serviceId) ?? false)

  function serviceName(id: string | null) {
    return id ? app.serviceName(id) : null
  }
</script>

<svelte:window onkeydown={(e) => c && e.key === 'Escape' && app.resolveConflict(false)} />

{#if c}
  <div class="backdrop">
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="pc-title">
      <h2 id="pc-title"><Icon name="warning" size={16} /> 포트가 이미 사용 중입니다</h2>
      <p class="lead">'{app.serviceName(c.id)}'이(가) 쓸 포트를 다른 프로세스가 점유하고 있습니다.</p>
      <ul class="owners">
        {#each c.owners as o (`${o.port}-${o.pid}`)}
          <li>
            <span class="port">:{o.port}</span>
            {#if serviceName(o.serviceId)}
              <span>대시보드 서비스 <b>{serviceName(o.serviceId)}</b></span>
            {:else}
              <span><b>{o.name}</b> <span class="muted">PID {o.pid}</span></span>
            {/if}
          </li>
        {/each}
      </ul>
      <p class="hint">
        {ownedByService
          ? '대시보드 서비스는 정상적으로 중지하고, 그 밖의 프로세스는 프로세스 트리째 강제 종료합니다.'
          : '점유한 프로세스를 프로세스 트리째 강제 종료합니다. 저장하지 않은 작업이 있다면 먼저 확인하세요.'}
      </p>
      <div class="actions">
        <button class="btn" onclick={() => app.resolveConflict(false)}>취소</button>
        <button class="btn danger" onclick={() => app.resolveConflict(true)}><Icon name="stop" size={13} /> 종료하고 시작</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 120;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.45);
  }
  .dialog {
    width: min(460px, calc(100vw - 32px));
    padding: 18px 20px 16px;
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    background: var(--panel);
    box-shadow: 0 16px 48px rgb(0 0 0 / 0.45);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
    font-size: 15px;
    color: var(--warn);
  }
  .lead {
    margin: 0 0 10px;
    color: var(--text-2);
  }
  .owners {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
  }
  .owners li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px;
    border-bottom: 1px solid var(--border);
  }
  .owners li:last-child {
    border-bottom: 0;
  }
  .port {
    min-width: 56px;
    font-family: var(--font-mono);
    color: var(--accent-text);
  }
  .muted {
    color: var(--faint);
    font-size: 12px;
  }
  .hint {
    margin: 0 0 14px;
    color: var(--faint);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 14px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .btn.danger {
    border-color: transparent;
    background: var(--err);
    color: #fff;
  }
</style>
