<script lang="ts">
  import { ArrowUpRight, CaseSensitive, ChevronRight, Search, TriangleAlert } from '@lucide/svelte';
  import { onDestroy, untrack } from 'svelte';
  import { api, type SearchDone, type SearchPlanned } from '../lib/api';
  import type { Match, QuickFilter, TableHits } from '../lib/bindings';
  import { connections } from '../lib/connections.svelte';
  import { findTable } from '../lib/data.svelte';
  import { visibilityOf } from '../lib/dbs.svelte';
  import { num } from '../lib/format';
  import { askConfirm } from '../lib/guard.svelte';
  import { BUDGETS, ROW_LIMITS, cellMatches, estimate, heavy, isNumber, kindsFor, rowFilters, searchTables, specOf, takeSearchSeed } from '../lib/search';
  import { duration } from '../lib/tools';
  import { errorText, Job } from '../lib/tools.svelte';
  import { keepSearch, keptSearch } from '../lib/search-kept';
  import { schemaShown } from '../lib/visibility';
  import { newTab, sessionFor, type Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import JobBar from './JobBar.svelte';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';

  let { tab, session }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const seed = untrack(() => takeSearchSeed());
  const schema = $derived(session.schema);
  const allNs = $derived(schema?.namespaces.map((n) => n.name) ?? []);
  const named = $derived(allNs.length > 1 || (allNs.length === 1 && allNs[0] !== ''));

  const was = untrack(() => keptSearch(tab));
  let value = $state(was?.value ?? '');
  let mode = $state<Match>(was?.mode ?? 'contains');
  let caseSensitive = $state(was?.caseSensitive ?? false);
  let kinds = $state(was?.kinds ?? kindsFor(''));
  let kindsTouched = $state(was?.kindsTouched ?? false);
  let chosen = $state<string[] | null>(was?.chosen ?? null);
  let filter = $state(was?.filter ?? '');
  let views = $state(was?.views ?? false);
  let rowLimit = $state<number>(was?.rowLimit ?? ROW_LIMITS[0]);
  let budget = $state<number>(was?.budget ?? BUDGETS[1]);
  let plan = $state.raw<SearchPlanned | null>(was?.plan ?? null);
  let results = $state.raw<TableHits[]>(was?.results ?? []);
  let failed = $state.raw<{ name: string; message: string }[]>(was?.failed ?? []);
  let done = $state.raw<SearchDone | null>(was?.done ?? null);
  let stopped = $state(was?.stopped ?? false);
  let failure = $state<string | null>(null);
  let current = $state('');
  let closed = $state<Set<string>>(was?.closed ?? new Set());
  let searched = $state.raw<ReturnType<typeof specOf> | null>(was?.searched ?? null);
  const job = new Job();

  $effect(() => {
    if (chosen || !schema) return;
    const v = visibilityOf(tab.connectionId);
    const shown = allNs.filter((n) => schemaShown(v, schema.current_database, n));
    untrack(() => (chosen = seed && allNs.includes(seed) ? [seed] : shown.length ? shown : allNs));
  });

  $effect(() => {
    const next = kindsFor(value);
    if (!untrack(() => kindsTouched)) kinds = next;
    else if (!next.number) kinds = { ...untrack(() => kinds), number: false };
  });

  const numeric = $derived(isNumber(value));
  const spec = $derived(specOf(value, mode, caseSensitive, kinds));
  const tables = $derived(searchTables(schema, chosen ?? [], filter, views));

  $effect(() => {
    const [s, t, sid] = [spec, tables, session.info.session_id];
    if (!s.value) {
      plan = null;
      return;
    }
    const timer = setTimeout(async () => {
      try {
        plan = await api.valueSearchPlan(sid, $state.snapshot(s), $state.snapshot(t));
      } catch (e) {
        failure = errorText(e);
      }
    }, 150);
    return () => clearTimeout(timer);
  });

  const cost = $derived(plan ? estimate(schema, tables, plan.columns) : null);
  const production = $derived(session.info.environment === 'production');
  const connectionName = $derived(connections.list.find((c) => c.id === tab.connectionId)?.name ?? '');

  function toggleKind(k: 'text' | 'number' | 'other') {
    kindsTouched = true;
    kinds = { ...kinds, [k]: !kinds[k] };
  }

  function toggleNs(n: string) {
    const now = chosen ?? [];
    chosen = now.includes(n) ? now.filter((x) => x !== n) : allNs.filter((x) => x === n || now.includes(x));
  }

  async function run(e?: SubmitEvent) {
    e?.preventDefault();
    if (!spec.value || !cost?.tables || job.running) return;
    if (production && heavy(cost)) {
      const ok = await askConfirm({
        title: m.search_heavy_title(),
        connection: connectionName,
        production: false,
        lines: [m.search_heavy_line({ tables: num(cost.tables), rows: num(cost.rows) })],
        risks: [],
        note: m.search_heavy_note({ time: duration(budget * 1000) }),
        action: m.search_heavy_go(),
      });
      if (ok === false) return;
    }
    const s = $state.snapshot(spec);
    searched = s;
    results = [];
    failed = [];
    done = null;
    stopped = false;
    failure = null;
    closed = new Set();
    try {
      done = await job.run((id, onProgress) =>
        api.valueSearch({ job_id: id, session_id: session.info.session_id, spec: s, tables: $state.snapshot(tables), row_limit: rowLimit, budget_secs: budget }, (ev) => {
          if (ev.kind === 'table') {
            current = ev.name;
            onProgress({ done: ev.index, total: ev.of, unit: 'tables', rows: 0, bytes: 0, stage: null });
          } else if (ev.kind === 'hits') results = [...results, ev.hits];
          else failed = [...failed, { name: ev.namespace ? `${ev.namespace}.${ev.name}` : ev.name, message: ev.message }];
        }),
      );
    } catch (err) {
      failure = errorText(err);
    }
  }

  onDestroy(() => {
    keepSearch(tab, { value, mode, caseSensitive, kinds: $state.snapshot(kinds), kindsTouched, chosen: chosen && [...chosen], filter, views, rowLimit, budget, plan, results, failed, done, stopped: stopped || job.running, closed, searched });
    void job.cancel();
  });

  function open(h: TableHits, quick: QuickFilter[]) {
    const info = findTable(session.schema, h.namespace, h.name);
    if (!info || !quick.length) return;
    const t = newTab(tab.connectionId);
    t.database = tab.database;
    t.view = 'data';
    void t.data.follow(session.info, info, quick);
  }

  function openColumn(h: TableHits, column: string) {
    if (!searched) return;
    open(h, [{ column, op: searched.mode === 'contains' ? 'contains' : 'eq', value: searched.value }]);
  }

  const keyOf = (h: TableHits) => `${h.namespace}.${h.name}`;
  function toggle(h: TableHits) {
    const next = new Set(closed);
    if (!next.delete(keyOf(h))) next.add(keyOf(h));
    closed = next;
  }

  const MODES = [
    { value: 'contains' as const, label: m.search_contains() },
    { value: 'exact' as const, label: m.search_exact() },
  ];
  const left = $derived(done ? done.of - done.searched : 0);
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_search()}</h2>
    <p class="hint">{m.search_lead()}</p>
  </div>
</header>

<form class="srch-form" onsubmit={(e) => void run(e)}>
  <div class="srch-line">
    <label class="field srch-value">
      <Search size={15} strokeWidth={1.8} aria-hidden="true" />
      <input type="search" bind:value placeholder={m.search_value_placeholder()} aria-label={m.search_value()} spellcheck="false" autocomplete="off" />
    </label>
    <Segmented label={m.search_mode()} value={mode} options={MODES} onchange={(v) => (mode = v)} />
    <button type="button" class="tool-chip srch-case" aria-pressed={caseSensitive} data-tip={m.search_case_hint()} onclick={() => (caseSensitive = !caseSensitive)}>
      <CaseSensitive size={15} strokeWidth={1.7} aria-hidden="true" />
      {m.search_case()}
    </button>
    <span class="spacer"></span>
    <button class="btn primary" type="submit" disabled={!spec.value || !cost?.tables || job.running}>
      <Search size={14} strokeWidth={1.8} aria-hidden="true" />
      {m.search_go()}
    </button>
  </div>

  <div class="srch-line">
    <span class="caps srch-label">{m.search_kinds()}</span>
    <div class="tool-chips" role="group" aria-label={m.search_kinds()}>
      <button type="button" class="tool-chip" aria-pressed={kinds.text} onclick={() => toggleKind('text')}>{m.search_kind_text()}</button>
      <button type="button" class="tool-chip" aria-pressed={kinds.number && numeric} disabled={!numeric} data-tip={numeric ? undefined : m.search_kind_number_off()} onclick={() => toggleKind('number')}>{m.search_kind_number()}</button>
      <button type="button" class="tool-chip" aria-pressed={kinds.other} onclick={() => toggleKind('other')}>{m.search_kind_other()}</button>
    </div>
    {#if named}
      <span class="caps srch-label">{m.search_schemas()}</span>
      <div class="tool-chips" role="group" aria-label={m.search_schemas()}>
        {#each allNs as n (n)}
          <button type="button" class="tool-chip mono" aria-pressed={chosen?.includes(n) ?? false} onclick={() => toggleNs(n)}>{n}</button>
        {/each}
      </div>
    {/if}
  </div>

  <div class="srch-line">
    <label class="srch-opt">
      <span class="caps srch-label">{m.search_tables()}</span>
      <span class="field srch-filter"><input bind:value={filter} placeholder={m.search_tables_placeholder()} spellcheck="false" autocomplete="off" /></span>
    </label>
    <label class="check srch-views" data-tip={m.search_views_hint()}><input type="checkbox" bind:checked={views} /> <span>{m.search_views()}</span></label>
    <label class="srch-opt">
      <span class="caps srch-label">{m.search_rows()}</span>
      <Select class="tool-select" label={m.search_rows()} bind:value={rowLimit} options={ROW_LIMITS.map((n) => ({ value: n, label: String(n) }))} />
    </label>
    <label class="srch-opt">
      <span class="caps srch-label">{m.search_budget()}</span>
      <Select class="tool-select" label={m.search_budget()} bind:value={budget} options={BUDGETS.map((s) => ({ value: s, label: duration(s * 1000) }))} />
    </label>
  </div>

  {#if !schema}
    <p class="hint srch-cost">{m.search_no_schema()}</p>
  {:else if cost}
    <div class="srch-cost">
      {#if cost.tables}
        <span class="hint">{m.search_estimate({ n: cost.tables, count: num(cost.tables), columns: num(cost.columns), rows: num(cost.rows) })}{#if cost.unknown}&nbsp;· {m.search_estimate_unknown({ count: num(cost.unknown) })}{/if}</span>
        {#if production}<span class="srch-prod">{m.search_prod()}</span>{/if}
        {#if plan?.sample}
          <details class="srch-sql">
            <summary class="hint">{m.search_sql()}</summary>
            <pre class="cmp-sql">{plan.sample}</pre>
          </details>
        {/if}
      {:else}
        <span class="hint">{m.search_estimate_none()}</span>
      {/if}
    </div>
  {/if}
</form>

{#if job.running}
  <div class="srch-job"><JobBar {job} label={current ? `${m.search_job()} · ${m.search_job_in({ table: current })}` : m.search_job()} /></div>
{/if}
{#if failure}<p class="tool-note err" role="alert">{failure}</p>{/if}

{#if done || results.length || stopped}
  <div class="tool-scroll srch-results">
    {#if stopped && !done}
      <p class="srch-summary" role="status"><span class="srch-stop"><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.search_stopped()}</span></p>
    {/if}
    {#if done}
      <p class="srch-summary" role="status">
        {#if done.with_hits}
          {m.search_summary({ n: done.with_hits, count: num(done.with_hits), searched: num(done.searched), rows: num(done.rows ?? 0), time: duration(done.elapsed_ms) })}
        {:else if !done.cancelled && !done.out_of_time}
          {m.search_nothing({ value: searched?.value ?? '' })}
        {/if}
        {#if done.cancelled}<span class="srch-stop"><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.search_cancelled({ count: num(left) })}</span>{/if}
        {#if done.out_of_time}<span class="srch-stop"><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.search_out_of_time({ count: num(left) })}</span>{/if}
      </p>
    {/if}
    {#each results as h (keyOf(h))}
      {@const open_ = !closed.has(keyOf(h))}
      {@const label = named ? `${h.namespace}.${h.name}` : h.name}
      <section class="srch-table" aria-label={label}>
        <div class="srch-table-head">
          <button type="button" class="srch-toggle" aria-expanded={open_} aria-label={open_ ? m.search_collapse({ table: label }) : m.search_expand({ table: label })} onclick={() => toggle(h)}>
            <ChevronRight size={14} strokeWidth={1.8} aria-hidden="true" />
            <span class="srch-name">{#if named}<span class="sub">{h.namespace}.</span>{/if}{h.name}</span>
            <span class="srch-count">{m.search_hits({ n: h.total ?? 0, count: num(h.total ?? 0) })}</span>
          </button>
          <div class="srch-cols">
            {#each h.columns as c (c.column)}
              <button type="button" class="tool-chip srch-col" data-tip={m.search_open_column({ table: h.name, column: c.column })} onclick={() => openColumn(h, c.column)}>
                <span class="mono">{c.column}</span><b>{num(c.count ?? 0)}</b>
              </button>
            {/each}
          </div>
        </div>
        {#if open_ && h.rows.length}
          <div class="srch-rows-wrap">
            <table class="tool-table srch-rows">
              <thead>
                <tr>
                  {#each h.shown as c (c)}<th class:key={h.key.includes(c)}>{c}</th>{/each}
                  <th class="acts"><span class="tool-sr">{m.search_open()}</span></th>
                </tr>
              </thead>
              <tbody>
                {#each h.rows as row, i (i)}
                  <tr>
                    {#each row as cell, j (j)}
                      {@const hit = h.columns.some((c) => c.column === h.shown[j]) && !!searched && cellMatches(searched, cell)}
                      <td class="mono" class:null={cell === null} class:hit>{cell ?? 'NULL'}</td>
                    {/each}
                    <td class="acts">
                      <button type="button" class="icon-btn" aria-label={m.search_open()} data-tip={m.search_open_hint()} onclick={() => searched && open(h, rowFilters(h, row, searched))}><ArrowUpRight size={14} strokeWidth={1.8} /></button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
          {#if (h.total ?? 0) > h.rows.length || !h.key.length}
            <p class="hint srch-foot">{#if (h.total ?? 0) > h.rows.length}{m.search_shown({ shown: num(h.rows.length), total: num(h.total ?? 0) })}{/if} {#if !h.key.length}{m.search_no_key()}{/if}</p>
          {/if}
        {/if}
      </section>
    {/each}
    {#if failed.length}
      <section class="srch-failed" aria-label={m.search_failed_head()}>
        <h3 class="caps">{m.search_failed_head()}</h3>
        <ul>
          {#each failed as f, i (i)}<li><span class="mono">{f.name}</span> <span class="hint">{f.message}</span></li>{/each}
        </ul>
      </section>
    {/if}
  </div>
{:else if !job.running && !failure}
  <p class="tool-empty">{m.search_idle()}</p>
{/if}
