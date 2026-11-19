<script lang="ts">
  import { app } from '../state.svelte'
  import Icon from './Icon.svelte'
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}" role={t.kind === 'error' ? 'alert' : 'status'}>
      <span class="text">{t.text}</span>
      <button title="닫기" onclick={() => app.dismiss(t.id)}><Icon name="x" size={12} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 16px;
    /* 설정 폼 하단 버튼줄을 가리지 않도록 그 위에 띄운다 */
    bottom: 76px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: min(460px, calc(100vw - 32px));
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--border-strong);
    border-left-width: 3px;
    border-radius: 8px;
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.3);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .toast.error {
    border-left-color: var(--err);
  }
  .toast.success {
    border-left-color: var(--ok);
  }
  .toast.info {
    border-left-color: var(--accent);
  }
  .text {
    flex: 1;
    word-break: break-word;
    user-select: text;
  }
  button {
    display: grid;
    place-items: center;
    padding: 2px;
    border: 0;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
</style>
