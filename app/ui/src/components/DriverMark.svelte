<script lang="ts">
  import type { Driver } from '../lib/drivers';
  let { driver, size = 20, framed = true }: { driver: Driver; size?: number; framed?: boolean } = $props();
  const view = $derived(('view' in driver.mark && driver.mark.view) || '0 0 24 24');
</script>

<svg class="mark" class:mono={framed && 'monogram' in driver.mark} width={size} height={size} viewBox={view} aria-hidden="true">
  {#if 'path' in driver.mark}
    <path d={driver.mark.path} />
  {:else}
    <text x="12" y="16.5" text-anchor="middle">{driver.mark.monogram}</text>
  {/if}
</svg>

<style>
  .mark {
    display: block;
    flex: none;
    fill: currentColor;
  }
  text {
    font: 700 12px var(--nd-font-ui);
    letter-spacing: -0.02em;
  }
  .mono {
    border-radius: min(var(--nd-radius), 3px);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, currentColor 50%, transparent);
  }
  .mono text {
    font: 600 10px var(--nd-font-mono);
    letter-spacing: 0;
  }
</style>
