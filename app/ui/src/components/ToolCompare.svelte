<script lang="ts">
  import { migrationFile } from '../lib/file-names';
  import { ArrowRight, ClipboardCopy, Download, GitCompareArrows, TextCursorInput, TriangleAlert } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { api, type Comparison } from '../lib/api';
  import type { DiffItem_Serialize as DiffItem, Dialect, DiffState, Note } from '../lib/bindings';
  import { connections } from '../lib/connections.svelte';
  import { copyText } from '../lib/contextmenu.svelte';
    import { num } from '../lib/format';
  import { DIALECT_NAME, diffGroups, scriptText } from '../lib/tools';
  import { errorText, toolsUi } from '../lib/tools.svelte';
  import { connect, newTab, sessionFor, workspace, type Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import Select from './Select.svelte';

  let { tab }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const nsOf = (id: string) => {
    const s = sessionFor(id)?.schema;
    return s?.namespaces.map((n) => n.name) ?? [];
  };
  const preferred = (id: string) => {
    const names = nsOf(id);
    return names.find((n) => n === 'public') ?? names[0] ?? '';
  };

  const sibling = untrack(() => {
    const me = connections.list.find((c) => c.id === tab.connectionId);
    const same = connections.list.filter((c) => c.id !== tab.connectionId && c.target.driver === me?.target.driver);
    return (same.find((c) => c.folder === me?.folder) ?? same[0] ?? me)?.id ?? tab.connectionId;
  });
  let source = $state({ id: untrack(() => tab.connectionId), ns: untrack(() => preferred(tab.connectionId)) });
  let target = $state({ id: sibling, ns: untrack(() => preferred(sibling)) });
  $effect(() => {
    const ask = toolsUi.compareTarget;
    if (!ask || ask.tab !== tab.id) return;
    untrack(() => {
      toolsUi.compareTarget = null;
      if (connections.list.some((c) => c.id === ask.target)) target = { id: ask.target, ns: preferred(ask.target) };
    });
  });
  let result = $state.raw<Comparison | null>(null);
  let failure = $state<string | null>(null);
  let busy = $state(false);
  let showSame = $state(false);
  let saved = $state<string | null>(null);

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
    }
  });

  const ready = $derived(!!sessionFor(source.id)?.schema && !!sessionFor(target.id)?.schema && !!source.ns && !!target.ns);
  const nameOf = (id: string) => connections.list.find((c) => c.id === id)?.name ?? id;
  const prodTarget = $derived(connections.list.find((c) => c.id === target.id)?.environment === 'production');
  const engine = (d: Dialect) => DIALECT_NAME[d];

  async function run() {
    const a = sessionFor(source.id);
    const b = sessionFor(target.id);
    if (!a || !b) return;
    busy = true;
    failure = null;
    saved = null;
    try {
      result = await api.schemaCompare({ source_session: a.info.session_id, source_namespace: source.ns, target_session: b.info.session_id, target_namespace: target.ns });
    } catch (e) {
      failure = errorText(e);
    } finally {
      busy = false;
    }
  }

  const counts = $derived.by(() => {
    const c: Record<DiffState, number> = { added: 0, removed: 0, changed: 0, same: 0 };
    for (const it of result?.items ?? []) if (!it.table) c[it.state]++;
    return c;
  });
  const groups = $derived(diffGroups(result?.items ?? []).filter((g) => showSame || g.head.state !== 'same'));
  const script = $derived(result ? scriptText(result.statements) : '');
  const notes = $derived(new Set((result?.items ?? []).flatMap((i) => i.notes)));

  const STATE: Record<DiffState, () => string> = { added: m.tool_diff_added, removed: m.tool_diff_removed, changed: m.tool_diff_changed, same: m.tool_diff_same };
  const KIND: Record<DiffItem['kind'], () => string> = {
    table: m.tool_kind_table,
    column: m.tool_kind_column,
    primary_key: m.tool_kind_pk,
    index: m.tool_kind_index,
    foreign_key: m.tool_kind_fk,
    view: m.tool_kind_view,
    function: m.tool_kind_function,
  };
  const NOTE: Record<Note, () => string> = { destructive: m.tool_note_destructive, rebuild: m.tool_note_rebuild, no_definition: m.tool_note_no_definition };

  async function save() {
    const name = migrationFile(target.ns);
    if (await api.exportSave(name, new TextEncoder().encode(`${script}\n`))) saved = name;
  }

  const oneLine = (s: string | null) => (s ? s.replace(/\s+/g, ' ').trim() : '');
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_compare()}</h2>
    <p class="hint">{m.tool_compare_lead()}</p>
  </div>
</header>

<div class="cmp-pick">
  {#each [{ side: source, label: m.tool_cmp_source(), hint: m.tool_cmp_source_hint() }, { side: target, label: m.tool_cmp_target(), hint: m.tool_cmp_target_hint() }] as p, i (i)}
    {#if i === 1}<ArrowRight class="cmp-arrow" size={16} strokeWidth={1.6} aria-hidden="true" />{/if}
    <fieldset class="cmp-side">
      <legend class="caps">{p.label}</legend>
      <Select class="tool-select" label={`${p.label} · ${m.tool_cmp_connection()}`} bind:value={p.side.id} options={connections.list.map((c) => ({ value: c.id, label: c.name }))} />
      <Select class="tool-select" label={`${p.label} · ${m.tool_kind_schema()}`} bind:value={p.side.ns} disabled={!nsOf(p.side.id).length} options={nsOf(p.side.id).map((n) => ({ value: n, label: n }))} />
      <span class="hint">{p.hint}</span>
    </fieldset>
  {/each}
  <button class="btn primary cmp-go" type="button" disabled={!ready || busy} onclick={() => void run()}>
    <GitCompareArrows size={14} strokeWidth={1.8} aria-hidden="true" />
    {busy ? m.tool_cmp_running() : m.tool_cmp_go()}
  </button>
</div>

{#if failure}<p class="tool-note err" role="alert">{failure}</p>{/if}

{#if result}
  <div class="cmp-body">
    <div class="cmp-summary">
      <span class="cmp-count added">{m.tool_cmp_count_added({ count: num(counts.added) })}</span>
      <span class="cmp-count removed">{m.tool_cmp_count_removed({ count: num(counts.removed) })}</span>
      <span class="cmp-count changed">{m.tool_cmp_count_changed({ count: num(counts.changed) })}</span>
      <span class="cmp-count same">{m.tool_cmp_count_same({ count: num(counts.same) })}</span>
      <span class="spacer"></span>
      <label class="check"><input type="checkbox" bind:checked={showSame} /> {m.tool_cmp_show_same()}</label>
    </div>
    <div class="cmp-grid" role="table" aria-label={m.tool_compare()}>
      <div class="cmp-row cmp-cols" role="row">
        <span role="columnheader">{m.tool_col_object()}</span>
        <span role="columnheader">{m.tool_cmp_source()} · {nameOf(source.id)} · {source.ns}</span>
        <span role="columnheader">{m.tool_cmp_target()} · {nameOf(target.id)} · {target.ns}</span>
      </div>
      {#each groups as g (g.head.kind + g.head.name)}
        {#each [g.head, ...g.children] as it, j (it.kind + it.name + j)}
          <div class="cmp-row {it.state}" class:child={j > 0} role="row">
            <span class="cmp-name" role="cell">
              <i class="cmp-mark" data-tip={STATE[it.state]()} aria-label={STATE[it.state]()}></i>
              <span class="sub">{KIND[it.kind]()}</span>
              <span class="mono">{it.name}</span>
              {#each it.notes as n (n)}<span class="tool-tag {n === 'destructive' ? 'bad' : 'warn'}">{NOTE[n]()}</span>{/each}
            </span>
            <span class="cmp-cell mono" role="cell" data-tip={it.source ?? ''}>{oneLine(it.source) || (it.state === 'removed' ? '—' : '')}</span>
            <span class="cmp-cell mono" role="cell" data-tip={it.target ?? ''}>{oneLine(it.target) || (it.state === 'added' ? '—' : '')}</span>
          </div>
        {/each}
      {:else}
        <p class="tool-none">{m.tool_cmp_equal()}</p>
      {/each}
    </div>
    <section class="cmp-script" aria-label={m.tool_cmp_script()}>
      <div class="cmp-script-head">
        <h3 class="caps">{m.tool_cmp_script()}</h3>
        <span class="hint">{m.tool_cmp_script_hint({ engine: engine(result.target_dialect), count: num(result.statements.length) })}</span>
        <span class="spacer"></span>
        <button class="btn ghost" type="button" disabled={!script} onclick={() => copyText(script)}><ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" /> {m.copy()}</button>
        <button class="btn ghost" type="button" disabled={!script} onclick={() => void save()}><Download size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_cmp_save()}</button>
        <button class="btn" type="button" disabled={!script} onclick={() => newTab(target.id, script)}><TextCursorInput size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_open_editor()}</button>
      </div>
      {#if result.source_dialect !== result.target_dialect || notes.has('rebuild') || notes.has('destructive') || prodTarget || saved}
        <ul class="cmp-warnings">
          {#if result.source_dialect !== result.target_dialect}<li><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_cmp_engines()}</li>{/if}
          {#if notes.has('destructive')}<li><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_cmp_destructive()}</li>{/if}
          {#if notes.has('rebuild')}<li><TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_cmp_rebuild()}</li>{/if}
          {#if prodTarget}<li class="prod">{m.tool_cmp_prod()}</li>{/if}
          {#if saved}<li class="ok">{m.tool_cmp_saved({ name: saved })}</li>{/if}
        </ul>
      {/if}
      <pre class="cmp-sql">{script || m.tool_cmp_nothing()}</pre>
    </section>
  </div>
{:else if openError}
  <p class="tool-empty err">{m.conn_open_failed()}: {openError.message}</p>
{:else if !busy}
  <p class="tool-empty">{ready ? m.tool_cmp_idle() : m.conn_connecting()}</p>
{/if}
