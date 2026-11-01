<script lang="ts">
  import { ArrowRight, Table2, X } from '@lucide/svelte';
  import type { FkAction } from '../lib/bindings';
  import { incoming, indexed, outgoing, type ErGraph, type ErTable } from '../lib/er/model';
  import { compact } from '../lib/er/render';
  import { m } from '../paraglide/messages';

  interface Props {
    graph: ErGraph;
    labels: string[];
    database: string;
    table: ErTable | null;
    onselect: (t: ErTable) => void;
    onopen: (id: string) => void;
    onarea: (index: number) => void;
    onclose: () => void;
  }
  let { graph, labels, database, table, onselect, onopen, onarea, onclose }: Props = $props();

  const out = $derived(table ? outgoing(graph, table.id) : []);
  const into = $derived(table ? incoming(graph, table.id) : []);
  const fkCols = $derived(new Set(table?.relation.foreign_keys.flatMap((f) => f.columns) ?? []));
  const refOf = (col: string) => table?.relation.foreign_keys.find((f) => f.columns.includes(col));
  const action = (a: FkAction) => a.replace('_', ' ');
  const go = (id: string) => {
    const t = graph.byId.get(id);
    if (t) onselect(t);
  };
</script>

<aside class="er-panel" aria-label={table ? table.name : m.er_overview({ database })}>
  {#if table}
    <header class="er-p-head">
      <p class="caps">{m.er_table_kind({ namespace: table.namespace })}</p>
      <button class="icon-btn" type="button" aria-label={m.er_close()} onclick={onclose}><X size={14} strokeWidth={1.8} /></button>
      <h2 class="display">{table.name}</h2>
    </header>
    <dl class="er-p-stats">
      <div><dt>{m.er_rows()}</dt><dd>{table.relation.estimated_rows === null ? '—' : compact(table.relation.estimated_rows)}</dd></div>
      <div><dt>{m.er_columns()}</dt><dd>{table.relation.columns.length}</dd></div>
      <div><dt>{m.er_links()}</dt><dd>{out.length + into.length}</dd></div>
    </dl>
    <div class="er-p-actions">
      <button class="btn primary" type="button" onclick={() => onopen(table.id)}>
        <Table2 size={13} strokeWidth={1.8} aria-hidden="true" />
        {m.er_open_data()}
      </button>
    </div>

    <section class="er-p-sec">
      <h3 class="caps">{m.er_columns()} <span>{table.relation.columns.length}</span></h3>
      <ul class="er-p-cols">
        {#each table.relation.columns as c (c.name)}
          {@const ref = refOf(c.name)}
          <li class:pk={c.primary_key}>
            <span class="er-mark">{c.primary_key ? 'PK' : fkCols.has(c.name) ? 'FK' : ''}</span>
            <span class="er-c-name">{c.name}</span>
            <span class="er-c-type">{c.type_name}</span>
            <span class="er-c-meta">
              {#if ref}{ref.ref_table}.{ref.ref_columns[ref.columns.indexOf(c.name)] ?? ''}{:else if !c.nullable}{m.er_not_null()}{/if}
            </span>
          </li>
        {/each}
      </ul>
    </section>

    <section class="er-p-sec">
      <h3 class="caps">{m.er_refs()} <span>{out.length}</span></h3>
      {#if out.length === 0}
        <p class="hint">{m.er_no_refs()}</p>
      {/if}
      <ul class="er-p-links">
        {#each out as e (e.id)}
          <li>
            <button type="button" onclick={() => go(e.to)}>
              <code>{e.fk.columns.join(', ')}</code>
              <ArrowRight size={11} strokeWidth={1.8} aria-hidden="true" />
              <code>{e.fk.ref_table}.{e.fk.ref_columns.join(', ')}</code>
            </button>
            <span class="er-l-meta">
              {#if !indexed(table.relation, e.fk.columns)}<em class="er-warn">{m.er_no_index()}</em>{/if}
              {#if e.fk.on_delete !== 'no_action'}on delete {action(e.fk.on_delete)}{/if}
            </span>
          </li>
        {/each}
      </ul>
    </section>

    <section class="er-p-sec">
      <h3 class="caps">{m.er_refd()} <span>{into.length}</span></h3>
      {#if into.length === 0}
        <p class="hint">{m.er_none_refd()}</p>
      {/if}
      <ul class="er-p-links">
        {#each into as e (e.id)}
          <li>
            <button type="button" onclick={() => go(e.from)}>
              <code>{graph.byId.get(e.from)?.name}.{e.fk.columns.join(', ')}</code>
            </button>
          </li>
        {/each}
      </ul>
    </section>

    <section class="er-p-sec">
      <h3 class="caps">{m.er_indexes()} <span>{table.relation.indexes.length}</span></h3>
      <ul class="er-p-idx">
        {#each table.relation.indexes as i (i.name)}
          <li><code>{i.name}</code> <span>({i.columns.join(', ')}){i.unique ? ` · ${m.er_unique()}` : ''}</span></li>
        {/each}
      </ul>
    </section>
  {:else}
    <header class="er-p-head">
      <p class="caps">{m.er_overview_kind()}</p>
      <h2 class="display">{database}</h2>
    </header>
    <dl class="er-p-stats">
      <div><dt>{m.er_title_tables()}</dt><dd>{graph.tables.length}</dd></div>
      <div><dt>{m.er_title_links()}</dt><dd>{graph.edges.length}</dd></div>
      <div><dt>{m.er_namespaces()}</dt><dd>{graph.namespaces.length}</dd></div>
    </dl>
    <section class="er-p-sec">
      <h3 class="caps">{m.er_areas()} <span>{graph.areas.length}</span></h3>
      <ul class="er-p-areas">
        {#each graph.areas as a (a.index)}
          <li>
            <button type="button" onclick={() => onarea(a.index)}>
              <i class="er-swatch" style:background="var(--er-area-{(a.index % 6) + 1})"></i>
              <span class="er-a-name">{labels[a.index]}</span>
              {#if graph.namespaces.length > 1 && labels[a.index] !== a.namespace}<span class="er-a-ns">{a.namespace}</span>{/if}
              <span class="er-a-count">{a.tables.length}</span>
            </button>
          </li>
        {/each}
      </ul>
    </section>
    <p class="hint er-p-hint">{m.er_hint()}</p>
  {/if}
</aside>
