<script lang="ts">
  import { api } from '../lib/api';
  import { updateErrorText } from '../lib/update.svelte';
  import { showToast } from '../lib/toast.svelte';
  import { m } from '../paraglide/messages';

  const URL = 'https://github.com/maxib0n';

  const GAP = 16;
  let bottom = $state(GAP);

  $effect(() => {
    const bar = document.querySelector('.statusbar');
    if (!bar) return;
    const fit = new ResizeObserver(() => (bottom = bar.getBoundingClientRect().height + GAP));
    fit.observe(bar);
    return () => fit.disconnect();
  });

  function open() {
    api.openLink(URL).catch((e: unknown) => showToast(updateErrorText(e), null, 'error'));
  }
</script>

<button type="button" class="maker" aria-label={m.maker_tip()} data-tip={m.maker_tip()} data-testid="maker" style:bottom="{bottom}px" onclick={open}>
  <i class="bunny" aria-hidden="true"></i>
</button>
