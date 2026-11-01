<script lang="ts">
  import { Search } from '@lucide/svelte';
  import { keys } from '../lib/contextmenu.svelte';
  import { openPalette, palette } from '../lib/palette.svelte';
  import { currentBindings } from '../lib/prefs.svelte';
  import { m } from '../paraglide/messages';

  const combo = $derived(currentBindings().palette);
  const shortcut = $derived(combo ? keys(combo) : '');
  const hint = $derived(shortcut ? m.pal_button_hint_keys({ keys: shortcut }) : m.pal_button_hint());
</script>

<button class="pal-trigger" type="button" data-tip={hint} aria-label={hint} aria-haspopup="dialog" aria-expanded={palette.open} onclick={() => openPalette()}>
  <Search size={13} strokeWidth={1.8} aria-hidden="true" />
  <span class="pal-trigger-label" aria-hidden="true">{m.pal_button()}</span>
  {#if shortcut}<kbd aria-hidden="true">{shortcut}</kbd>{/if}
</button>
