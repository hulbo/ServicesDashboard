<script lang="ts" module>
  import type { IconName } from './Icon.svelte'

  export interface MenuItem {
    label: string
    icon?: IconName
    shortcut?: string
    danger?: boolean
    disabled?: boolean
    action: () => void
  }
  export type MenuEntry = MenuItem | 'separator'
</script>

<script lang="ts">
  import Icon from './Icon.svelte'

  let { x, y, items, onclose }: { x: number; y: number; items: MenuEntry[]; onclose: () => void } = $props()

  let menu: HTMLDivElement | undefined = $state()
  let pos = $state({ left: 0, top: 0 })

  $effect(() => {
    // 화면 밖으로 나가지 않게 위치를 보정한다.
    const el = menu
    if (!el) return
    const r = el.getBoundingClientRect()
    pos = {
      left: Math.max(4, Math.min(x, window.innerWidth - r.width - 4)),
      top: Math.max(4, Math.min(y, window.innerHeight - r.height - 4)),
    }
    el.focus()
  })

  function run(item: MenuItem) {
    if (item.disabled) return
    onclose()
    item.action()
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape') onclose()
  }}
/>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="backdrop" onclick={onclose} oncontextmenu={(e) => { e.preventDefault(); onclose() }}></div>
<div class="menu" role="menu" tabindex="-1" bind:this={menu} style="left:{pos.left}px; top:{pos.top}px">
  {#each items as item, i (i)}
    {#if item === 'separator'}
      <div class="sep"></div>
    {:else}
      <button class="item" class:danger={item.danger} role="menuitem" disabled={item.disabled} onclick={() => run(item)}>
        <span class="ico">{#if item.icon}<Icon name={item.icon} size={14} />{/if}</span>
        <span class="label">{item.label}</span>
        {#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
  }
  .menu {
    position: fixed;
    z-index: 91;
    min-width: 200px;
    padding: 4px;
    background: var(--panel);
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
    outline: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .item:hover:not(:disabled) {
    background: var(--accent);
    color: #fff;
  }
  .item:disabled {
    color: var(--faint);
    cursor: default;
  }
  .item.danger:not(:hover) {
    color: var(--err);
  }
  .ico {
    width: 14px;
    display: flex;
  }
  .label {
    flex: 1;
  }
  .shortcut {
    color: var(--muted);
    font-size: 11px;
  }
  .item:hover .shortcut {
    color: inherit;
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border-strong);
  }
</style>
