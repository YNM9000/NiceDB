<script lang="ts">
  import { Undo2 } from '@lucide/svelte';
  import { m } from '../paraglide/messages';

  let { own, onreset }: { own: boolean; onreset: () => void } = $props();
</script>

{#if own}
  <span class="own-mark" data-tip={m.model_own_title()}>
    <span class="own-tag">{m.model_own()}</span>
    <button class="icon-btn own-reset" type="button" aria-label={m.model_reset()} data-tip={m.model_reset()} onclick={onreset}><Undo2 size={12} strokeWidth={1.9} /></button>
  </span>
{/if}
