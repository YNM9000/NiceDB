<script lang="ts">
  import { rowDiffFile } from '../lib/file-names';
  import { ArrowRight, ClipboardCopy, Download, Rows3, TextCursorInput, TriangleAlert } from '@lucide/svelte';
  import { onDestroy, untrack } from 'svelte';
  import { api, type RowCompared, type RowSource } from '../lib/api';
  import type { Column, RowState } from '../lib/bindings';
  import { connections } from '../lib/connections.svelte';
  import { copyText } from '../lib/contextmenu.svelte';
  import { num } from '../lib/format';
  import { DiffSet } from '../lib/rowdiff.svelte';
  import { columnNotes, commonColumns, EDITOR_STATEMENTS, primaryKey, proposeKey, STATES, tableOfSql, type SourceMode } from '../lib/rowdiff';
  import { DIALECT_NAME, duration, scriptText } from '../lib/tools';
  import { errorText, Job, takeRowSeed } from '../lib/tools.svelte';
  import { connect, newTab, sessionFor, workspace, type Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import Grid from './Grid.svelte';
  import JobBar from './JobBar.svelte';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';

  let { tab }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const PREVIEW = 200;

  const schemaOf = (id: string) => sessionFor(id)?.schema ?? null;
  const nsOf = (id: string) => schemaOf(id)?.namespaces.map((n) => n.name) ?? [];
  const preferred = (id: string) => {
    const names = nsOf(id);
    return names.find((n) => n === 'public') ?? names[0] ?? '';
  };
  const tablesOf = (id: string, ns: string) => schemaOf(id)?.namespaces.find((n) => n.name === ns)?.tables.map((t) => t.name) ?? [];

  interface Side {
    id: string;
    mode: SourceMode;
    sql: string;
    ns: string;
    table: string;
    columns: Column[] | null;
    error: string | null;
  }

  const seed = untrack(() => takeRowSeed(tab.id));
  const sibling = untrack(() => {
    const me = connections.list.find((c) => c.id === tab.connectionId);
    const same = connections.list.filter((c) => c.id !== tab.connectionId && c.target.driver === me?.target.driver);
    return (same.find((c) => c.folder === me?.folder) ?? same[0] ?? me)?.id ?? tab.connectionId;
  });
  const fresh = (id: string): Side => ({ id, mode: seed ? 'query' : 'table', sql: seed ?? '', ns: '', table: '', columns: null, error: null });
  let source = $state(fresh(untrack(() => tab.connectionId)));
  let target = $state(fresh(sibling));
  let key = $state<string[]>([]);
  let set = $state.raw<DiffSet | null>(null);
  let done = $state.raw<RowCompared | null>(null);
  let failure = $state<string | null>(null);
  let compareId: string | null = null;
  let statements = $state.raw<string[] | null>(null);
  let scriptNs = $state('');
  let scriptTable = $state('');
  let saved = $state<string | null>(null);
  const job = new Job();

  $effect(() => {
    for (const id of [source.id, target.id]) {
      if (!sessionFor(id) && !workspace.opening[id] && !workspace.openErrors[id]) untrack(() => void connect(id));
    }
  });

  const openError = $derived([source.id, target.id].map((id) => workspace.openErrors[id]).find(Boolean) ?? null);

  $effect(() => {
    for (const side of [source, target]) {
      const names = nsOf(side.id);
      if (names.length && !names.includes(side.ns)) side.ns = preferred(side.id);
      const tables = tablesOf(side.id, side.ns);
      if (tables.length && !tables.includes(side.table)) side.table = (tables[0] ?? '');
    }
  });

  function sourceOf(side: Side): RowSource | null {
    const s = sessionFor(side.id);
    if (!s) return null;
    if (side.mode === 'query') return side.sql.trim() ? { kind: 'query', session_id: s.info.session_id, sql: side.sql } : null;
    return side.table ? { kind: 'table', session_id: s.info.session_id, namespace: side.ns, name: side.table } : null;
  }

  function probe(side: Side) {
    const src = sourceOf(side);
    const sig = JSON.stringify(src);
    const timer = setTimeout(
      async () => {
        if (!src) return;
        try {
          const p = await api.rowProbe(src);
          if (JSON.stringify(sourceOf(side)) === sig) {
            side.columns = p.columns;
            side.error = null;
          }
        } catch (e) {
          if (JSON.stringify(sourceOf(side)) === sig) {
            side.columns = null;
            side.error = errorText(e);
          }
        }
      },
      side.mode === 'query' ? 450 : 0,
    );
    return () => clearTimeout(timer);
  }

  $effect(() => {
    void [source.id, source.mode, source.sql, source.ns, source.table, sessionFor(source.id)];
    return untrack(() => probe(source));
  });
  $effect(() => {
    void [target.id, target.mode, target.sql, target.ns, target.table, sessionFor(target.id)];
    return untrack(() => probe(target));
  });

  const common = $derived(target.columns && source.columns ? commonColumns(target.columns, source.columns) : []);

  function pkOf(side: Side): { table: string; columns: string[] } {
    const ref = side.mode === 'table' ? { namespace: side.ns, name: side.table } : tableOfSql(side.sql);
    if (!ref?.name) return { table: '', columns: [] };
    return { table: ref.name, columns: primaryKey(schemaOf(side.id), ref.namespace, ref.name) };
  }

  const pk = $derived.by(() => {
    const t = pkOf(target);
    return t.columns.length ? t : pkOf(source);
  });
  const proposed = $derived(proposeKey(common, pk.columns));
  const proposedFromPk = $derived(pk.columns.length > 0 && proposed.length === pk.columns.length);

  $effect(() => {
    void common.join('\u0000');
    key = untrack(() => proposed);
  });

  function toggleKey(c: string) {
    key = key.includes(c) ? key.filter((k) => k !== c) : common.filter((x) => x === c || key.includes(x));
  }

  const ready = $derived(!!sourceOf(source) && !!sourceOf(target) && key.length > 0 && !!source.columns && !!target.columns);
  const nameOf = (id: string) => connections.list.find((c) => c.id === id)?.name ?? id;
  const prodTarget = $derived(connections.list.find((c) => c.id === target.id)?.environment === 'production');

  function defaultTable() {
    const ref = target.mode === 'table' ? { namespace: target.ns, name: target.table } : tableOfSql(target.sql);
    scriptNs = ref?.namespace ?? preferred(target.id);
    scriptTable = ref?.name ?? '';
  }

  async function run() {
    const a = sourceOf(target);
    const b = sourceOf(source);
    if (!a || !b) return;
    if (compareId) void api.rowForget(compareId).catch(() => {});
    const next = new DiffSet();
    set = next;
    done = null;
    failure = null;
    statements = null;
    saved = null;
    defaultTable();
    try {
      let read = { a: 0, b: 0 };
      done = await job.run((id, onProgress) => {
        compareId = id;
        return api.rowCompare({ job_id: id, a, b, key }, (e) => {
          if (e.kind === 'plan') next.setPlan(e.plan);
          else if (e.kind === 'rows') next.add(e.rows);
          else {
            read = { a: e.a_rows ?? 0, b: e.b_rows ?? 0 };
            onProgress({ done: read.a + read.b, total: null, unit: 'rows', rows: 0, bytes: 0, stage: null });
          }
        });
      });
      if (done.plan.common.length && !next.plan) next.setPlan(done.plan);
    } catch (e) {
      failure = errorText(e);
      set = null;
    }
  }

  $effect(() => {
    const id = compareId;
    if (!done || done.cancelled || !id || !scriptTable.trim()) {
      statements = null;
      return;
    }
    const [ns, name] = [scriptNs, scriptTable];
    const timer = setTimeout(async () => {
      try {
        statements = await api.rowScript(id, ns, name);
      } catch (e) {
        failure = errorText(e);
      }
    }, 200);
    return () => clearTimeout(timer);
  });

  onDestroy(() => {
    void job.cancel();
    if (compareId) void api.rowForget(compareId).catch(() => {});
  });

  const script = $derived(statements ? scriptText(statements) : '');
  const preview = $derived(statements ? scriptText(statements.slice(0, PREVIEW)) : '');
  const notes = $derived(columnNotes(set?.plan ?? null));
  const engine = $derived(done ? DIALECT_NAME[done.a_dialect] : '');

  const LABEL: Record<RowState, (p: { count: string }) => string> = { added: m.rows_count_added, removed: m.rows_count_removed, changed: m.rows_count_changed, same: m.rows_count_same };
  const HINT: Record<RowState, () => string> = { added: m.rows_added_hint, removed: m.rows_removed_hint, changed: m.rows_changed_hint, same: m.rows_same_hint };
  const count = (s: RowState) => (done ? (done.counts[s] ?? 0) : (set?.got[s] ?? 0));
  const differences = $derived(done ? (done.counts.added ?? 0) + (done.counts.removed ?? 0) + (done.counts.changed ?? 0) : 1);

  async function save() {
    const name = rowDiffFile(scriptTable);
    if (await api.exportSave(name, new TextEncoder().encode(`${script}\n`))) saved = name;
  }

  const MODES = [
    { value: 'table' as const, label: m.rows_mode_table() },
    { value: 'query' as const, label: m.rows_mode_query() },
  ];
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_rows()}</h2>
    <p class="hint">{m.rows_lead()}</p>
  </div>
</header>

<div class="cmp-pick rows-pick">
  {#each [{ side: source, label: m.tool_cmp_source(), hint: m.rows_source_hint() }, { side: target, label: m.tool_cmp_target(), hint: m.rows_target_hint() }] as p, i (i)}
    {#if i === 1}<ArrowRight class="cmp-arrow" size={16} strokeWidth={1.6} aria-hidden="true" />{/if}
    <fieldset class="cmp-side rows-side">
      <legend class="caps">{p.label}</legend>
      <Select class="tool-select" label={`${p.label} · ${m.tool_cmp_connection()}`} bind:value={p.side.id} options={connections.list.map((c) => ({ value: c.id, label: c.name }))} />
      <Segmented label={`${p.label} · ${m.rows_mode()}`} value={p.side.mode} options={MODES} onchange={(v) => (p.side.mode = v)} />
      {#if p.side.mode === 'query'}
        <textarea class="tool-sql rows-sql" aria-label={`${p.label} · ${m.rows_sql()}`} spellcheck="false" placeholder={m.rows_ph_sql()} bind:value={p.side.sql}></textarea>
      {:else}
        <Select class="tool-select" label={`${p.label} · ${m.tool_kind_schema()}`} bind:value={p.side.ns} disabled={!nsOf(p.side.id).length} options={nsOf(p.side.id).map((n) => ({ value: n, label: n }))} />
        <Select class="tool-select" label={`${p.label} · ${m.rows_table()}`} bind:value={p.side.table} disabled={!tablesOf(p.side.id, p.side.ns).length} options={tablesOf(p.side.id, p.side.ns).map((t) => ({ value: t, label: t }))} />
      {/if}
      {#if p.side.error}<span class="hint err" role="alert">{p.side.error}</span>{:else}<span class="hint">{p.hint}</span>{/if}
    </fieldset>
  {/each}
</div>

<div class="rows-key">
  <span class="caps">{m.rows_key()}</span>
  {#if common.length}
    <div class="tool-chips" role="group" aria-label={m.rows_key()}>
      {#each common as c (c)}
        <button type="button" class="tool-chip mono" aria-pressed={key.includes(c)} onclick={() => toggleKey(c)}>{c}</button>
      {/each}
    </div>
    <span class="hint">{key.length === 0 ? m.rows_key_none() : proposedFromPk && key.join() === proposed.join() ? m.rows_key_pk({ table: pk.table }) : m.rows_key_hint()}</span>
  {:else}
    <span class="hint">{m.rows_key_wait()}</span>
  {/if}
  <span class="spacer"></span>
  <button class="btn primary" type="button" disabled={!ready || job.running} onclick={() => void run()}>
    <Rows3 size={14} strokeWidth={1.8} aria-hidden="true" />
    {m.rows_go()}
  </button>
</div>

{#if job.running}<div class="rows-job"><JobBar {job} label={m.rows_job()} /></div>{/if}
{#if failure}<p class="tool-note err" role="alert">{failure}</p>{/if}

{#if set?.view}
  {@const view = set.view}
  <div class="cmp-body rows-body">
    <div class="cmp-summary rows-summary">
      <div class="rows-filters" role="group" aria-label={m.rows_filter()}>
        {#each STATES as s (s)}
          <button type="button" class="tool-chip cmp-count {s}" aria-pressed={set.show[s]} data-tip={HINT[s]()} onclick={() => set?.toggle(s)}>{LABEL[s]({ count: num(count(s)) })}</button>
        {/each}
      </div>
      <span class="spacer"></span>
      <span class="hint rows-cols">
        {m.rows_cols_common({ count: num(set.plan?.common.length ?? 0) })}{#if notes.onlyB.length}&nbsp;· <span class="rows-warn">{m.rows_cols_only_source({ names: notes.onlyB.join(', ') })}</span>{/if}{#if notes.onlyA.length}&nbsp;· <span class="rows-warn">{m.rows_cols_only_target({ names: notes.onlyA.join(', ') })}</span>{/if}{#if notes.retyped.length}&nbsp;· <span class="rows-warn">{m.rows_cols_retyped({ list: notes.retyped.map((r) => `${r.name} (${r.a} ↔ ${r.b})`).join(', ') })}</span>{/if}
      </span>
    </div>
    <div class="rows-grid" style:height={view.count > 0 ? `${Math.min(view.count, 16) * 24 + 34}px` : undefined}>
      {#if view.count > 0}
        {#key view}<Grid result={view} edit={view.edit} />{/key}
      {:else if done && !done.cancelled}
        <p class="tool-none">{differences === 0 ? m.rows_equal() : m.rows_none_shown()}</p>
      {/if}
    </div>
    <section class="cmp-script" aria-label={m.rows_script()}>
      <div class="cmp-script-head">
        <h3 class="caps">{m.rows_script()}</h3>
        {#if done && !done.cancelled}
          <div class="rows-target">
            <span class="sub">{m.rows_script_table()}</span>
            <span class="field rows-name ns"><input class="mono" aria-label={`${m.rows_script_table()} · ${m.tool_kind_schema()}`} bind:value={scriptNs} spellcheck="false" autocomplete="off" /></span>
            <span aria-hidden="true">.</span>
            <span class="field rows-name"><input class="mono" aria-label={m.rows_script_table()} bind:value={scriptTable} spellcheck="false" autocomplete="off" /></span>
          </div>
          {#if statements}<span class="hint">{m.rows_script_hint({ engine, count: num(statements.length) })}</span>{/if}
        {/if}
        <span class="spacer"></span>
        <button class="btn ghost" type="button" disabled={!script} onclick={() => copyText(script)}><ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" /> {m.copy()}</button>
        <button class="btn ghost" type="button" disabled={!script} onclick={() => void save()}><Download size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_cmp_save()}</button>
        <button class="btn" type="button" disabled={!script} onclick={() => newTab(target.id, script)}><TextCursorInput size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_open_editor()}</button>
      </div>
      {#if done}
        <ul class="cmp-warnings">
          <li class="rows-read">{m.rows_read({ target: num(done.a_rows ?? 0), source: num(done.b_rows ?? 0), time: duration(done.elapsed_ms) })} {nameOf(target.id)} → {nameOf(source.id)}</li>
          {#if done.cancelled}<li><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.rows_cancelled()}</li>{/if}
          {#if done.same_cut}<li>{m.rows_same_cut({ count: num(view.count) })}</li>{/if}
          {#if done.a_dialect !== done.b_dialect}<li><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_cmp_engines()}</li>{/if}
          {#if notes.onlyA.length || notes.onlyB.length}<li><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.rows_script_cols()}</li>{/if}
          {#if statements && statements.length > EDITOR_STATEMENTS}<li><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.rows_script_big({ count: num(statements.length) })}</li>{/if}
          {#if prodTarget}<li class="prod">{m.tool_cmp_prod()}</li>{/if}
          {#if saved}<li class="ok">{m.tool_cmp_saved({ name: saved })}</li>{/if}
        </ul>
      {/if}
      <pre class="cmp-sql">{#if statements}{preview || m.tool_cmp_nothing()}{#if statements.length > PREVIEW}{`\n\n${m.rows_preview_cut({ count: num(statements.length - PREVIEW) })}`}{/if}{/if}</pre>
    </section>
  </div>
{:else if openError}
  <p class="tool-empty err">{m.conn_open_failed()}: {openError.message}</p>
{:else if !job.running && !failure}
  <p class="tool-empty">{m.rows_idle()}</p>
{/if}
