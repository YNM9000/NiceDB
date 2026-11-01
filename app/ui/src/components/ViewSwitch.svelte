<script lang="ts">
  import { m } from '../paraglide/messages';
  import { activeTab, sessionOf } from '../lib/workspace.svelte';
  import { designOf } from '../lib/design.svelte';
  import { viewsOf } from '../lib/family';
  import Segmented from './Segmented.svelte';

  const tab = $derived(activeTab());
  const session = $derived(tab ? sessionOf(tab) : null);
  const ready = $derived(!!tab && !!session && !designOf(tab));
  const ALL = [
    { value: 'query', label: m.view_query() },
    { value: 'data', label: m.view_data() },
    { value: 'schema', label: m.view_schema() },
    { value: 'tools', label: m.view_tools() },
  ] as const;
  const options = $derived(session ? ALL.filter((o) => viewsOf(session.info.caps).includes(o.value)) : ALL);
</script>

{#if tab && ready && options.length > 1}
  <Segmented class="view-switch" label={m.view_switch()} value={tab.view} {options} onchange={(v) => (tab.view = v)} />
{/if}
