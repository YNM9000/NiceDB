<script lang="ts">
  import { CircleArrowUp } from '@lucide/svelte';
  import { update } from '../lib/update.svelte';
  import { m } from '../paraglide/messages';
  import UpdateRelease from './UpdateRelease.svelte';

  let { ondetails }: { ondetails: () => void } = $props();
</script>

{#if update.banner}
  <section class="upd-banner" aria-label={m.upd_banner_label()} data-testid="update-banner">
    <span class="upd-mark" aria-hidden="true"><CircleArrowUp size={16} strokeWidth={1.7} /></span>
    <UpdateRelease compact {ondetails} />
  </section>
{/if}
