<script lang="ts">
  import type { RunState } from '../types'
  import { STATE_LABELS } from '../types'

  let { state = 'stopped', size = 9 }: { state?: RunState; size?: number } = $props()
</script>

<span class="dot st-{state}" style="--size:{size}px" title={STATE_LABELS[state]}></span>

<style>
  .dot {
    flex: none;
    width: var(--size);
    height: var(--size);
    border-radius: 50%;
    background: var(--idle);
    box-sizing: border-box;
  }
  .st-running {
    background: var(--ok);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--ok) 25%, transparent);
  }
  .st-starting,
  .st-stopping {
    background: var(--warn);
    animation: pulse 1s ease-in-out infinite;
  }
  .st-waiting {
    background: transparent;
    border: 1.5px dashed var(--accent-text);
    animation: pulse 1.4s ease-in-out infinite;
  }
  .st-failed {
    background: var(--err);
  }
  .st-exited {
    background: transparent;
    border: 1.5px solid var(--idle);
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
