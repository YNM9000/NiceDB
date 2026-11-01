<script lang="ts">
  import { ListFilter } from '@lucide/svelte';
  import type { ErGraph } from '../lib/er/model';
  import { m } from '../paraglide/messages';

  interface Props {
    graph: ErGraph;
    labels: string[];
    hiddenNs: ReadonlySet<string>;
    hiddenAreas: ReadonlySet<number>;
    onchange: (ns: ReadonlySet<string>, areas: ReadonlySet<number>) => void;
  }
  let { graph, labels, hiddenNs, hiddenAreas, onchange }: Props = $props();
  let open = $state(false);
  let root: HTMLDivElement;

  const groups = $derived(graph.namespaces.map((ns) => ({ ns, areas: graph.areas.filter((a) => a.namespace === ns) })));
  const active = $derived(hiddenNs.size + hiddenAreas.size > 0);

  function toggle<T>(set: ReadonlySet<T>, v: T): Set<T> {
    const next = new Set(set);
    if (!next.delete(v)) next.add(v);
    return next;
  }

  $effect(() => {
    if (!open) return;
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent ? e.key === 'Escape' : !root.contains(e.target as Node)) open = false;
    };
    window.addEventListener('pointerdown', close);
    window.addEventListener('keydown', close);
    return () => {
      window.removeEventListener('pointerdown', close);
      window.removeEventListener('keydown', close);
    };
  });
</script>

<div class="menu er-filter" bind:this={root}>
  <button class="btn ghost" type="button" aria-haspopup="true" aria-expanded={open} aria-pressed={active} onclick={() => (open = !open)}>
    <ListFilter size={13} strokeWidth={1.8} aria-hidden="true" />
    {active ? m.er_filter_some({ count: graph.tables.length - graph.tables.filter((t) => hiddenNs.has(t.namespace) || hiddenAreas.has(t.area)).length }) : m.er_filter()}
  </button>
  {#if open}
    <div class="menu-list er-filter-list" role="group" aria-label={m.er_filter()}>
      <button type="button" disabled={!active} onclick={() => onchange(new Set(), new Set())}>{m.er_filter_all()}</button>
      {#each groups as g (g.ns)}
        <label class="er-f-ns">
          <input type="checkbox" checked={!hiddenNs.has(g.ns)} onchange={() => onchange(toggle(hiddenNs, g.ns), hiddenAreas)} />
          <span>{g.ns}</span>
        </label>
        {#if g.areas.length > 1}
          {#each g.areas as a (a.index)}
            <label class="er-f-area">
              <input type="checkbox" disabled={hiddenNs.has(g.ns)} checked={!hiddenAreas.has(a.index)} onchange={() => onchange(hiddenNs, toggle(hiddenAreas, a.index))} />
              <i class="er-swatch" style:background="var(--er-area-{(a.index % 6) + 1})"></i>
              <span>{labels[a.index]}</span>
              <small>{a.tables.length}</small>
            </label>
          {/each}
        {/if}
      {/each}
    </div>
  {/if}
</div>
