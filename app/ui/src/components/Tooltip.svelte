<script lang="ts">
  import { float } from '../lib/float';
  import { placeTip } from '../lib/menu-place';
  import { hide, installTooltips, tip } from '../lib/tooltip.svelte';

  $effect(() => installTooltips());
</script>

{#if tip.anchor}
  {#key tip.anchor}
    <div id="nd-tip" class="float nd-tip" role="tooltip" use:float={{ anchor: tip.anchor, place: placeTip, onaway: hide }}>{tip.text}</div>
  {/key}
{/if}
