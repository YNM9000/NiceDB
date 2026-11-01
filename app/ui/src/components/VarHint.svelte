<script lang="ts">
  import { m } from '../paraglide/messages';
  import type { VarPreviewer } from '../lib/vars-preview.svelte';
  import { hasVars, varMessage } from '../lib/vars';

  let { text, preview }: { text: string; preview: VarPreviewer } = $props();

  const shown = $derived(hasVars(text) ? preview.of(text) : undefined);
</script>

{#if shown?.kind === 'value'}
  <p class="var-hint" aria-label={m.var_becomes({ value: shown.text })}><span class="var-arrow" aria-hidden="true">→</span> <span class="var-value">{shown.text || '∅'}</span></p>
{:else if shown?.kind === 'problem'}
  <p class="var-hint bad" role="status">{varMessage(shown.error)}</p>
{/if}
