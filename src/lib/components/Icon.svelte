<script lang="ts" module>
  export type IconName =
    | 'play'
    | 'stop'
    | 'restart'
    | 'plus'
    | 'folder'
    | 'folder-plus'
    | 'chevron-right'
    | 'chevron-down'
    | 'trash'
    | 'copy'
    | 'gear'
    | 'search'
    | 'arrow-down'
    | 'refresh'
    | 'external'
    | 'x'
    | 'clear'
    | 'stop-all'
    | 'play-all'
    | 'edit'
    | 'file'
    | 'terminal'
    | 'workspace'
    | 'import'
    | 'runtime'
    | 'warning'
    | 'bug'
    | 'link'
    | 'branch'
    | 'grip'
    | 'more'
</script>

<script lang="ts">
  let { name, size = 16, title }: { name: IconName; size?: number; title?: string } = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.4"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden={title ? undefined : 'true'}
  role={title ? 'img' : undefined}
  class="icon icon-{name}"
>
  {#if title}<title>{title}</title>{/if}
  {#if name === 'play'}
    <path d="M4.5 3.2v9.6L12.5 8z" fill="currentColor" stroke="none" />
  {:else if name === 'stop'}
    <rect x="4" y="4" width="8" height="8" rx="1" fill="currentColor" stroke="none" />
  {:else if name === 'restart'}
    <path d="M13 8a5 5 0 1 1-1.5-3.6" />
    <path d="M12 2v3h-3" />
  {:else if name === 'plus'}
    <path d="M8 3v10M3 8h10" />
  {:else if name === 'folder'}
    <path d="M2 4.5c0-.6.4-1 1-1h3l1.5 1.5H13c.6 0 1 .4 1 1V12c0 .6-.4 1-1 1H3c-.6 0-1-.4-1-1z" />
  {:else if name === 'folder-plus'}
    <path d="M2 4.5c0-.6.4-1 1-1h3l1.5 1.5H13c.6 0 1 .4 1 1V12c0 .6-.4 1-1 1H3c-.6 0-1-.4-1-1z" />
    <path d="M8 7.5v3.5M6.25 9.25h3.5" />
  {:else if name === 'chevron-right'}
    <path d="M6 4l4 4-4 4" />
  {:else if name === 'chevron-down'}
    <path d="M4 6l4 4 4-4" />
  {:else if name === 'trash'}
    <path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5" />
  {:else if name === 'copy'}
    <rect x="5.5" y="5.5" width="7.5" height="7.5" rx="1" />
    <path d="M3 10.5V4c0-.6.4-1 1-1h6.5" />
  {:else if name === 'gear'}
    <circle cx="8" cy="8" r="2" />
    <path
      d="M8 1.8v1.6M8 12.6v1.6M14.2 8h-1.6M3.4 8H1.8M12.4 3.6l-1.1 1.1M4.7 11.3l-1.1 1.1M12.4 12.4l-1.1-1.1M4.7 4.7L3.6 3.6"
    />
  {:else if name === 'search'}
    <circle cx="7" cy="7" r="4" />
    <path d="M10 10l3.5 3.5" />
  {:else if name === 'arrow-down'}
    <path d="M8 3v10M4 9l4 4 4-4" />
  {:else if name === 'refresh'}
    <path d="M13 3v3.5H9.5" />
    <path d="M12.6 6.5A5 5 0 1 0 13 9.5" />
  {:else if name === 'external'}
    <path d="M9 3h4v4M13 3L7.5 8.5M11 9.5V13H3V5h3.5" />
  {:else if name === 'x'}
    <path d="M4 4l8 8M12 4l-8 8" />
  {:else if name === 'clear'}
    <circle cx="8" cy="8" r="5" />
    <path d="M4.5 11.5l7-7" />
  {:else if name === 'stop-all'}
    <rect x="2.5" y="4.5" width="7" height="7" rx="1" fill="currentColor" stroke="none" />
    <path d="M12 4.5v7M14 4.5v7" />
  {:else if name === 'play-all'}
    <path d="M2.5 3.5v9l6-4.5z" fill="currentColor" stroke="none" />
    <path d="M8.5 3.5v9l6-4.5z" fill="currentColor" stroke="none" />
  {:else if name === 'edit'}
    <path d="M10.5 3l2.5 2.5L6 12.5H3.5V10z" />
  {:else if name === 'file'}
    <path d="M4 2h5l3 3v9H4z" />
    <path d="M9 2v3h3" />
  {:else if name === 'terminal'}
    <rect x="2" y="3" width="12" height="10" rx="1" />
    <path d="M4.5 6l2 2-2 2M8 10.5h3.5" />
  {:else if name === 'workspace'}
    <rect x="2" y="2.5" width="12" height="11" rx="1.2" />
    <path d="M6 2.5v11" />
  {:else if name === 'import'}
    <path d="M8 2v8M5 7l3 3 3-3" />
    <path d="M2.5 10.5v2.5h11v-2.5" />
  {:else if name === 'runtime'}
    <rect x="4" y="4" width="8" height="8" rx="1" />
    <path d="M6 2v2M10 2v2M6 12v2M10 12v2M2 6h2M2 10h2M12 6h2M12 10h2" />
  {:else if name === 'bug'}
    <ellipse cx="8" cy="9" rx="3.5" ry="4.5" />
    <path d="M8 4.5V13.5M4.5 9H2M14 9h-2.5M5 5.5L3.5 4M11 5.5L12.5 4M5 12.5L3.5 14M11 12.5l1.5 1.5" />
  {:else if name === 'link'}
    <path d="M6.5 9.5l3-3M7 4.5l1-1a2.8 2.8 0 0 1 4 4l-1 1M9 11.5l-1 1a2.8 2.8 0 0 1-4-4l1-1" />
  {:else if name === 'branch'}
    <circle cx="5" cy="3.5" r="1.5" />
    <circle cx="5" cy="12.5" r="1.5" />
    <circle cx="11" cy="4.5" r="1.5" />
    <path d="M5 5v6M11 6c0 3.2-6 2.6-6 5" />
  {:else if name === 'more'}
    <g fill="currentColor" stroke="none"><circle cx="3.5" cy="8" r="1.3" /><circle cx="8" cy="8" r="1.3" /><circle cx="12.5" cy="8" r="1.3" /></g>
  {:else if name === 'grip'}
    <g fill="currentColor" stroke="none">
      <circle cx="6" cy="4" r="1.1" /><circle cx="10" cy="4" r="1.1" />
      <circle cx="6" cy="8" r="1.1" /><circle cx="10" cy="8" r="1.1" />
      <circle cx="6" cy="12" r="1.1" /><circle cx="10" cy="12" r="1.1" />
    </g>
  {:else if name === 'warning'}
    <path d="M8 2.5L14 13H2z" />
    <path d="M8 6.5v3M8 11.3v.2" />
  {/if}
</svg>

<style>
  .icon {
    flex: none;
    display: block;
  }
</style>
