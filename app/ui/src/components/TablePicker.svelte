<script lang="ts">
  import { ChevronDown, Eye, Table2 } from '@lucide/svelte';
  import type { Schema } from '../lib/api';
  import { compact } from '../lib/format';
  import { m } from '../paraglide/messages';
  import Popover from './Popover.svelte';

  interface Props {
    schema: Schema | null;
    current: { namespace: string; name: string } | null;
    onpick: (namespace: string, name: string) => void;
  }
  let { schema, current, onpick }: Props = $props();
  let open = $state(false);
  let filter = $state('');

  const q = $derived(filter.trim().toLowerCase());
  const items = $derived(
    (schema?.namespaces ?? []).flatMap((ns) => [
      ...ns.tables.map((r) => ({ ns: ns.name, name: r.name, rows: r.estimated_rows, view: false })),
      ...ns.views.map((r) => ({ ns: ns.name, name: r.name, rows: r.estimated_rows, view: true })),
    ]),
  );
  const shown = $derived(items.filter((i) => !q || i.name.toLowerCase().includes(q)));
  const many = $derived((schema?.namespaces.length ?? 0) > 1);
</script>

<Popover label={m.data_pick_table()} bind:open class="dv-picker">
  {#snippet trigger()}
    <span class="dv-name">{current?.name ?? m.data_pick_table()}</span>
    <ChevronDown size={14} strokeWidth={1.8} aria-hidden="true" />
  {/snippet}
  <label class="field">
    <!-- svelte-ignore a11y_autofocus -->
    <input type="search" placeholder={m.data_tables_filter()} aria-label={m.data_tables_filter()} bind:value={filter} autofocus />
  </label>
  <ul class="dv-tables" role="listbox" aria-label={m.data_pick_table()}>
    {#each shown as t (`${t.ns}.${t.name}`)}
      <li role="option" aria-selected={current?.namespace === t.ns && current.name === t.name}>
        <button
          type="button"
          onclick={() => {
            open = false;
            onpick(t.ns, t.name);
          }}
        >
          {#if t.view}<Eye size={13} strokeWidth={1.6} aria-hidden="true" />{:else}<Table2 size={13} strokeWidth={1.6} aria-hidden="true" />{/if}
          <span class="tree-name">{many ? `${t.ns}.` : ''}{t.name}</span>
          {#if t.rows}<span class="tree-count">{compact(t.rows)}</span>{/if}
        </button>
      </li>
    {/each}
  </ul>
</Popover>
