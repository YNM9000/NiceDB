<script lang="ts">
  import { m } from '../paraglide/messages';
  import { extraOf, type ColumnExtra, type ColumnFacts, type Describe } from '../lib/colinfo';

  interface Props {
    facts: ColumnFacts;
    describe?: Describe | null;
  }
  let { facts, describe = null }: Props = $props();

  let extra = $state<ColumnExtra | null>(null);
  let loading = $state(false);

  $effect(() => {
    const d = describe;
    const f = facts;
    extra = null;
    if (!d) return;
    let live = true;
    loading = true;
    void d(f.ns, f.table).then((design) => {
      if (!live) return;
      extra = extraOf(design, f.column.name);
      loading = false;
    });
    return () => {
      live = false;
    };
  });

  const type = $derived(extra?.type || facts.column.type_name);
</script>

<div class="col-card" data-testid="column-card">
  <div class="cc-head">
    <span class="cc-name"><span class="cc-table">{facts.table}.</span>{facts.column.name}</span>
    {#if facts.column.primary_key}<span class="cc-badge">{m.colinfo_pk()}</span>{/if}
  </div>
  <div class="cc-type">
    <span class="cc-mono">{type}</span>
    <span class="cc-dot">·</span>
    <span>{facts.column.nullable ? m.colinfo_null() : m.colinfo_not_null()}</span>
    {#if extra?.identity}<span class="cc-dot">·</span><span>{m.colinfo_identity()}</span>{/if}
  </div>
  <dl class="cc-facts">
    {#if extra?.default}<dt>{m.colinfo_default()}</dt><dd class="cc-mono">{extra.default}</dd>{/if}
    {#if extra?.comment}<dt>{m.colinfo_comment()}</dt><dd class="cc-comment">{extra.comment}</dd>{/if}
    {#if facts.points.length}<dt>{m.colinfo_points()}</dt><dd class="cc-mono">{facts.points.join(', ')}</dd>{/if}
    {#if facts.pointedBy.length}<dt>{m.colinfo_pointed_by()}</dt><dd class="cc-mono">{facts.pointedBy.join(', ')}</dd>{/if}
    {#if facts.indexes.length}
      <dt>{m.colinfo_indexes()}</dt>
      <dd>
        {#each facts.indexes as x (x.name)}
          <div class="cc-index"><span class="cc-mono">{x.name}</span>{#if x.unique}<span class="cc-tag">{m.colinfo_unique()}</span>{/if}{#if x.columns.length > 1}<span class="cc-cols">({x.columns.join(', ')})</span>{/if}</div>
        {/each}
      </dd>
    {/if}
  </dl>
  {#if loading}<div class="cc-note">{m.colinfo_loading()}</div>{/if}
</div>

<style>
  .col-card {
    min-width: 220px;
    max-width: 360px;
    padding: 9px 12px 10px;
    font-family: var(--nd-font-ui);
    font-size: 12px;
    line-height: 1.45;
    color: var(--nd-ink);
  }
  .cc-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .cc-name {
    font-family: var(--nd-font-mono);
    font-weight: 600;
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
  .cc-table {
    color: var(--nd-ink-3);
    font-weight: 400;
  }
  .cc-badge,
  .cc-tag {
    flex: none;
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 0 5px;
    border-radius: var(--nd-radius);
    color: var(--nd-accent-text);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--nd-accent) 45%, transparent);
    white-space: nowrap;
  }
  .cc-badge {
    margin-left: auto;
  }
  .cc-type {
    margin-top: 2px;
    color: var(--nd-ink-2);
  }
  .cc-dot {
    color: var(--nd-ink-3);
    margin: 0 3px;
  }
  .cc-mono {
    font-family: var(--nd-font-mono);
    overflow-wrap: anywhere;
  }
  .cc-facts {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 3px 12px;
    margin: 8px 0 0;
  }
  .cc-facts:empty {
    display: none;
  }
  dt {
    color: var(--nd-ink-3);
  }
  dd {
    margin: 0;
    min-width: 0;
  }
  .cc-comment {
    font-style: italic;
  }
  .cc-index {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 5px;
  }
  .cc-cols {
    color: var(--nd-ink-3);
    font-family: var(--nd-font-mono);
  }
  .cc-note {
    margin-top: 6px;
    color: var(--nd-ink-3);
  }
</style>
