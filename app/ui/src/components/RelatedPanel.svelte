<script lang="ts">
  import { ArrowRight, SquareArrowOutUpRight, X } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { api, type Schema, type SessionInfo } from '../lib/api';
  import type { QuickFilter } from '../lib/bindings';
  import { findTable, type TableInfo } from '../lib/data.svelte';
  import { backwardFilters, filterText, forwardFilters, incoming, labelColumns, outgoing, targetOf } from '../lib/fk';
  import { num } from '../lib/format';
  import { queued, rowsOf, Stale } from '../lib/related';
  import { m } from '../paraglide/messages';
  import Resizer from './Resizer.svelte';

  interface Props {
    session: SessionInfo;
    schema: Schema | null;
    info: TableInfo;
    row: (string | null)[] | null;
    label: string | null;
    onfollow: (target: TableInfo, quick: QuickFilter[], newTab: boolean) => void;
    onclose: () => void;
  }
  let { session, schema, info, row, label, onfollow, onclose }: Props = $props();

  type Out = { key: string; target: TableInfo | null; via: string; quick: QuickFilter[] | null; preview: string | null | undefined };
  type In = { key: string; target: TableInfo | null; via: string; quick: QuickFilter[] | null; count: number | null | undefined };

  const names = $derived(info.columns.map((c) => c.name));
  const out = $derived<Out[]>(
    outgoing(schema, info).map((fk) => {
      const t = targetOf(info.namespace, fk);
      return { key: fk.name, target: findTable(schema, t.namespace, t.name), via: fk.columns.join(', '), quick: row && forwardFilters(fk, names, row), preview: undefined };
    }),
  );
  const ins = $derived<In[]>(
    incoming(schema, info).map((x) => ({ key: `${x.namespace}.${x.table}.${x.fk.name}`, target: findTable(schema, x.namespace, x.table), via: x.fk.columns.join(', '), quick: row && backwardFilters(x.fk, names, row), count: undefined })),
  );

  let previews = $state<Record<string, string | null>>({});
  let counts = $state<Record<string, number | null>>({});
  let failure = $state<string | null>(null);
  let seq = 0;

  async function load(outs: Out[], inc: In[]) {
    const mine = ++seq;
    const stale = () => mine !== seq;
    failure = null;
    previews = {};
    counts = {};
    try {
      for (const o of outs) {
        if (!o.target || !o.quick) continue;
        const shown = labelColumns(o.target.columns, o.quick.map((q) => q.column));
        const page = await rowsOf(session.session_id, o.target, shown.length ? shown : [o.quick[0]?.column ?? ''], o.quick, null, 1, stale);
        if (mine !== seq) return;
        const cells = page.rows[0];
        previews = { ...previews, [o.key]: cells ? cells.filter((v) => v !== null && v !== '').join(' · ') || filterText(o.quick) : null };
      }
      const wanted = inc.filter((x) => x.target && x.quick);
      if (!wanted.length) return;
      const n = await queued(session.session_id, () =>
        api.dataRelated(
          session.session_id,
          wanted.map((x) => ({
            table: { namespace: x.target?.namespace ?? '', name: x.target?.name ?? '' },
            types: (x.quick ?? []).map((q) => ({ name: q.column, type_name: x.target?.columns.find((c) => c.name === q.column)?.type_name ?? '' })),
            key: (x.quick ?? []).map((q) => ({ column: q.column, value: q.value ?? null })),
          })),
        ),
        stale,
      );
      if (mine !== seq) return;
      counts = Object.fromEntries(wanted.map((x, i) => [x.key, n[i] ?? null]));
    } catch (e) {
      if (mine === seq && !(e instanceof Stale)) failure = e instanceof Error ? e.message : String(e);
    }
  }

  const signature = $derived(JSON.stringify([info.namespace, info.name, out.map((o) => o.quick), ins.map((x) => x.quick)]));

  $effect(() => {
    void signature;
    const [outs, inc] = untrack(() => [out, ins] as const);
    const t = setTimeout(() => void load(outs, inc), 180);
    return () => clearTimeout(t);
  });

  const rows = (n: number | null | undefined) => (n === undefined ? '…' : n === null ? '—' : n === 0 ? m.fk_rows_none() : n === 1 ? m.fk_rows_one() : m.fk_rows_other({ count: num(n) }));
</script>

<aside class="related" aria-label={m.fk_related()}>
  <Resizer pane="related" side="start" label={m.fk_resize()} />
  <header class="history-head">
    <h2 class="caps">{m.fk_related()}</h2>
    {#if label}<span class="related-row">{label}</span>{/if}
    <span class="spacer"></span>
    <button class="icon-btn" type="button" aria-label={m.fk_close()} onclick={onclose}><X size={14} strokeWidth={1.8} /></button>
  </header>
  {#if !row}
    <p class="history-empty">{m.fk_no_row()}</p>
  {:else}
    <div class="related-body">
      {#if failure}<p class="related-failure" role="alert">{failure}</p>{/if}
      <section class="related-sec" aria-labelledby="related-out">
        <h3 id="related-out" class="related-title">{m.fk_points_to()}</h3>
        {#if out.length === 0}
          <p class="related-none">{m.fk_none_out()}</p>
        {:else}
          <ul class="related-list">
            {#each out as o (o.key)}
              {@const preview = previews[o.key]}
              <li class="related-item">
                <button class="related-go" type="button" disabled={!o.target || !o.quick} onclick={() => o.target && o.quick && onfollow(o.target, o.quick, false)}>
                  <span class="related-name">{o.target?.name ?? '?'}</span>
                  <span class="related-via">{m.fk_via({ columns: o.via })}</span>
                  <span class="related-what" class:muted={!o.quick || preview === null}>
                    {#if !o.quick}{m.fk_null()}{:else if preview === undefined}…{:else if preview === null}{m.fk_missing()}{:else}{preview}{/if}
                  </span>
                  <ArrowRight class="related-arrow" size={14} strokeWidth={1.8} aria-hidden="true" />
                </button>
                <button class="icon-btn related-tab" type="button" aria-label={m.fk_follow_tab()} data-tip={m.fk_follow_tab()} disabled={!o.target || !o.quick} onclick={() => o.target && o.quick && onfollow(o.target, o.quick, true)}><SquareArrowOutUpRight size={13} strokeWidth={1.8} /></button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
      <section class="related-sec" aria-labelledby="related-in">
        <h3 id="related-in" class="related-title">{m.fk_pointed_by()}</h3>
        {#if ins.length === 0}
          <p class="related-none">{m.fk_none_in()}</p>
        {:else}
          <ul class="related-list">
            {#each ins as x (x.key)}
              {@const n = counts[x.key]}
              <li class="related-item">
                <button class="related-go" type="button" disabled={!x.target || !x.quick} onclick={() => x.target && x.quick && onfollow(x.target, x.quick, false)}>
                  <span class="related-name">{x.target?.name ?? '?'}</span>
                  <span class="related-via">{m.fk_via({ columns: x.via })}</span>
                  <span class="related-count" class:zero={n === 0} data-tip={n === null ? m.fk_rows_unknown() : undefined}>{rows(n)}</span>
                  <ArrowRight class="related-arrow" size={14} strokeWidth={1.8} aria-hidden="true" />
                </button>
                <button class="icon-btn related-tab" type="button" aria-label={m.fk_follow_tab()} data-tip={m.fk_follow_tab()} disabled={!x.target || !x.quick} onclick={() => x.target && x.quick && onfollow(x.target, x.quick, true)}><SquareArrowOutUpRight size={13} strokeWidth={1.8} /></button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    </div>
  {/if}
</aside>
