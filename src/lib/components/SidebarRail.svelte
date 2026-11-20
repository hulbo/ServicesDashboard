<script lang="ts">
  // 사이드바를 자동 숨김으로 두었을 때 왼쪽에 남는 기둥.
  // 실행 중인 서비스를 아이콘으로 보여 주고, 마우스를 올리면 서비스 목록이 펼쳐진다.
  import { badgeTitle, serviceBadge } from '../rail'
  import { app } from '../state.svelte'
  import { STATE_LABELS } from '../types'
  import Icon from './Icon.svelte'
  import StatusDot from './StatusDot.svelte'

  let { onenter, onleave, onmove, onopen }: { onenter: () => void; onleave: () => void; onmove: () => void; onopen: () => void } = $props()

  const active = $derived(app.activeServices)
  const selectedId = $derived(app.selection?.kind === 'service' ? app.selection.id : null)
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="rail" onmouseenter={onenter} onmouseleave={onleave} onmousemove={onmove}>
  <button class="tool" title="사이드바 고정 (Ctrl+B)" aria-label="사이드바 고정" onclick={() => app.toggleSidebarPinned()}>
    <Icon name="pin" />
  </button>
  <button class="tool" title="서비스 목록 (마우스를 올려도 열림)" aria-label="서비스 목록" onclick={onopen}>
    <Icon name="list" />
  </button>
  <div class="count" class:on={active.length > 0} title="실행 중인 서비스 {active.length}개">{active.length}</div>
  <div class="list">
    {#each active as s (s.id)}
      {@const st = app.statuses[s.id]}
      <button
        class="svc"
        class:selected={selectedId === s.id}
        style:--c={app.colorOf(s.id)}
        title={badgeTitle(s.name, STATE_LABELS[st?.state ?? 'stopped'], st?.ports ?? [])}
        onclick={() => app.select({ kind: 'service', id: s.id })}
      >
        <span class="abbr">{serviceBadge(s.name)}</span>
        <span class="dot"><StatusDot state={st?.state} size={8} /></span>
      </button>
    {/each}
  </div>
</div>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    min-height: 0;
    padding: 6px 0;
    background: var(--panel);
    border-right: 1px solid var(--border);
  }
  .tool {
    display: grid;
    place-items: center;
    width: 30px;
    height: 28px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .tool:hover {
    background: var(--hover);
    color: var(--text);
  }
  .count {
    margin: 4px 0 2px;
    min-width: 22px;
    padding: 1px 0;
    border-top: 1px solid var(--border);
    color: var(--faint);
    font-size: 10.5px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .count.on {
    color: var(--ok);
  }
  .list {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: none;
    padding: 2px 0;
  }
  /* 테두리는 통합 로그의 서비스 이름과 같은 색(--c) */
  .svc {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border: 1.5px solid var(--c, var(--border-strong));
    border-radius: 7px;
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .svc:hover {
    background: color-mix(in srgb, var(--c) 14%, var(--panel-2));
  }
  .svc.selected {
    background: color-mix(in srgb, var(--c) 22%, var(--panel-2));
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--c) 45%, transparent);
  }
  .abbr {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: -0.2px;
    white-space: nowrap;
  }
  .dot {
    position: absolute;
    right: -3px;
    bottom: -3px;
    display: grid;
    padding: 2px;
    border-radius: 50%;
    background: var(--panel);
  }
</style>
