<script lang="ts">
  import { CircleCheck, FileUp, TriangleAlert } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { api, type DataImported, type DataImportSample } from '../lib/api';
  import type { FileFormat, OnRowError } from '../lib/bindings';
  import { connections } from '../lib/connections.svelte';
  import { num } from '../lib/format';
  import { duration, fileName, formatOf, matchColumns, plainName, tableFromFile } from '../lib/tools';
  import { confirmToolWrite, errorText, Job, type ImportAsk } from '../lib/tools.svelte';
  import { refreshSchema, sessionFor } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import JobBar from './JobBar.svelte';
  import Segmented from './Segmented.svelte';
  import WriteLock from './WriteLock.svelte';
  import Select from './Select.svelte';

  let { ask, onclose }: { ask: ImportAsk; onclose: () => void } = $props();

  const session = $derived(sessionFor(ask.connectionId, ask.database ?? null));
  const schema = $derived(session?.schema ?? null);
  const production = $derived(session?.info.environment === 'production');
  const tables = $derived((schema?.namespaces ?? []).flatMap((n) => n.tables.map((t) => ({ ns: n.name, rel: t }))));
  const defaultNs = untrack(() => sessionFor(ask.connectionId, ask.database ?? null)?.schema?.namespaces[0]?.name ?? '');

  let path = $state<string | null>(null);
  let format = $state<FileFormat>('csv');
  let header = $state(true);
  let sheet = $state<string | null>(null);
  let sample = $state.raw<DataImportSample | null>(null);
  let mode = $state<'existing' | 'create'>(untrack(() => (ask.table ? 'existing' : 'create')));
  let picked = $state(untrack(() => (ask.table ? `${ask.table.namespace}.${ask.table.name}` : '')));
  let mapping = $state<(number | null)[]>([]);
  let newName = $state('');
  let newNs = $state(defaultNs);
  let columns = $state<{ include: boolean; name: string; type: string }[]>([]);
  let emptyIsNull = $state(true);
  let onError = $state<OnRowError>('stop');
  let failure = $state<string | null>(null);
  let result = $state.raw<DataImported | null>(null);
  let target = $state('');
  const job = new Job();
  let dialog: HTMLDialogElement;

  const existing = $derived(tables.find((t) => `${t.ns}.${t.rel.name}` === picked) ?? null);

  $effect(() => dialog.showModal());

  $effect(() => {
    if (mode === 'existing' && !existing && tables[0]) picked = `${tables[0].ns}.${tables[0].rel.name}`;
  });

  $effect(() => {
    const names = sample?.columns.map((c) => c.name) ?? [];
    const rel = existing?.rel;
    untrack(() => (mapping = rel ? matchColumns(names, rel.columns.map((c) => c.name)) : []));
  });

  async function choose() {
    const p = await api.pickOpen(['csv', 'tsv', 'json', 'ndjson', 'jsonl', 'xlsx', 'xls', 'ods', 'parquet'], m.tool_import_files());
    if (!p) return;
    path = p;
    format = formatOf(p) ?? 'csv';
    sheet = null;
    newName = tableFromFile(p);
    await load();
  }

  async function load() {
    if (!path || !session) return;
    failure = null;
    try {
      sample = await api.dataImportSample(session.info.session_id, { path, format, header, sheet });
      columns = sample.columns.map((c) => ({ include: true, name: plainName(c.name) || c.name, type: c.type_name }));
    } catch (e) {
      sample = null;
      failure = errorText(e);
    }
  }

  function close() {
    if (job.running) void job.cancel();
    dialog.close();
    onclose();
  }

  const mapped = $derived(mode === 'existing' ? mapping.filter((x) => x !== null).length : columns.filter((c) => c.include && c.name.trim()).length);

  async function start() {
    if (result?.committed) return close();
    if (!session || !path || !sample) return;
    failure = null;
    result = null;
    const name = connections.list.find((c) => c.id === ask.connectionId)?.name ?? '';
    const into = mode === 'existing' ? (existing?.rel.name ?? '') : newName.trim();
    target = into;
    try {
      const typed = await confirmToolWrite(session.info, m.tool_import_confirm({ table: into }), [fileName(path), name], m.tool_import_go(), m.tool_import_note(), false);
      if (typed === false) return;
      const req =
        mode === 'existing' && existing
          ? {
              target: { kind: 'existing' as const, namespace: existing.ns, name: existing.rel.name },
              mapping: existing.rel.columns.flatMap((c, i) => (mapping[i] === null || mapping[i] === undefined ? [] : [{ source: mapping[i], target: c.name }])),
            }
          : {
              target: { kind: 'create' as const, namespace: newNs, name: into, columns: columns.filter((c) => c.include && c.name.trim()).map((c) => ({ name: c.name.trim(), type_name: c.type, nullable: true })) },
              mapping: columns.flatMap((c, i) => (c.include && c.name.trim() ? [{ source: i, target: c.name.trim() }] : [])),
            };
      const out = await job.run((job_id, onProgress) =>
        api.dataImport({ job_id, session_id: session.info.session_id, source: { path: path ?? '', format, header, sheet }, ...req, empty_is_null: emptyIsNull, on_error: onError, confirm: typed }, onProgress),
      );
      result = out;
      if (out.committed) void refreshSchema(ask.connectionId, ask.database ?? null);
    } catch (e) {
      failure = errorText(e);
    }
  }
</script>

<dialog class="sheet-dialog tool-dialog wide" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="di-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      void start();
    }}
  >
    <header class="dialog-head">
      <h2 id="di-title" class="display">{ask.table ? m.tool_import_into({ table: ask.table.name }) : m.tool_import_title()}</h2>
      <p class="hint">{path ? fileName(path) : m.tool_import_lead()}</p>
    </header>
    <div class="dialog-body">
      {#if result}
        {#if result.committed}
          <p class="tool-done big" role="status"><CircleCheck size={16} strokeWidth={1.8} aria-hidden="true" /> {m.tool_import_done({ rows: num(result.inserted ?? 0), table: target, time: duration(result.elapsed_ms) })}</p>
          {#if result.skipped}<p class="tool-warn">{m.tool_import_skipped({ count: num(result.skipped ?? 0) })}</p>{/if}
        {:else if result.cancelled}
          <p class="tool-warn" role="status">{m.tool_import_cancelled()}</p>
        {:else}
          <p class="tool-warn big" role="alert"><TriangleAlert size={16} strokeWidth={1.8} aria-hidden="true" /> {m.tool_import_rolled_back({ line: num(result.faults[0]?.line ?? 0) })}</p>
        {/if}
        {#if result.faults.length}
          <h3 class="caps tool-sub">{m.tool_import_faults()}</h3>
          <ol class="di-faults">
            {#each result.faults as f (f.line)}<li><span class="mono">{m.tool_import_line({ line: num(f.line ?? 0) })}</span> {f.message}</li>{/each}
          </ol>
          {#if result.faults_cut}<p class="hint">{m.tool_import_faults_cut()}</p>{/if}
        {/if}
      {:else if !path}
        <div class="di-start">
          <button class="btn primary" type="button" onclick={() => void choose()}><FileUp size={14} strokeWidth={1.8} aria-hidden="true" /> {m.tool_import_choose()}</button>
          <p class="hint">{m.tool_import_formats()}</p>
        </div>
      {:else}
        <div class="di-file">
          <span class="tool-tag">{format.toUpperCase()}</span>
          <span class="mono di-path" data-tip={path}>{path}</span>
          <button class="btn ghost small" type="button" disabled={job.running} onclick={() => void choose()}>{m.tool_import_other()}</button>
          {#if format === 'csv' || format === 'tsv'}<label class="check"><input type="checkbox" bind:checked={header} onchange={() => void load()} /> {m.tool_import_header()}</label>{/if}
          {#if format === 'xlsx' && sample && sample.sheets.length > 1}
            <Select class="tool-select" label={m.tool_import_sheet()} bind:value={sheet} onchange={() => void load()} options={sample.sheets.map((s) => ({ value: s, label: s }))} />
          {/if}
        </div>
        {#if sample}
          <div class="di-preview">
            <table class="tool-table">
              <thead>
                <tr>{#each sample.columns as c (c.name)}<th>{c.name}<span class="sub">{c.type_name}</span></th>{/each}</tr>
              </thead>
              <tbody>
                {#each sample.rows.slice(0, 5) as r, i (i)}
                  <tr>{#each sample.columns as _, j (j)}<td class="mono">{r[j] ?? ''}</td>{/each}</tr>
                {/each}
              </tbody>
            </table>
          </div>
          <p class="hint">{sample.more ? m.tool_import_preview_more({ count: Math.min(5, sample.rows.length) }) : m.tool_import_preview({ count: sample.rows.length })}</p>

          <div class="form-field">
            <span class="caps">{m.tool_import_target()}</span>
            <Segmented
              label={m.tool_import_target()}
              value={mode}
              options={[
                { value: 'existing', label: m.tool_import_existing() },
                { value: 'create', label: m.tool_import_create() },
              ] as const}
              onchange={(v) => (mode = v)}
            />
          </div>
          {#if mode === 'existing'}
            <Select class="tool-select" label={m.tool_kind_table()} bind:value={picked} options={tables.map((t) => ({ value: `${t.ns}.${t.rel.name}`, label: (schema?.namespaces.length ?? 1) > 1 ? `${t.ns}.${t.rel.name}` : t.rel.name }))} />
            {#if existing}
              <div class="di-map" role="table" aria-label={m.tool_import_mapping()}>
                <div class="di-map-row di-map-head" role="row">
                  <span role="columnheader">{m.tool_import_head_table()}</span>
                  <span role="columnheader"></span>
                  <span role="columnheader">{m.tool_import_head_file()}</span>
                </div>
                {#each existing.rel.columns as c, i (c.name)}
                  <div class="di-map-row" role="row">
                    <span role="cell" class="mono">{c.name}<span class="sub">{c.type_name}{c.nullable ? '' : ' · NOT NULL'}</span></span>
                    <span role="cell" class="di-from">←</span>
                    <span role="cell" class="di-pick"><Select class="tool-select" label={m.tool_import_from({ column: c.name })} bind:value={mapping[i]} options={[{ value: null, label: m.tool_import_skip_col() }, ...sample.columns.map((s, j) => ({ value: j, label: s.name }))]} /></span>
                  </div>
                {/each}
              </div>
            {/if}
          {:else}
            <div class="form-row two">
              <label class="form-field">
                <span class="caps">{m.tool_import_table_name()}</span>
                <span class="field"><input bind:value={newName} spellcheck="false" autocomplete="off" /></span>
              </label>
              {#if (schema?.namespaces.length ?? 0) > 1}
                <label class="form-field">
                  <span class="caps">{m.tool_kind_schema()}</span>
                  <Select class="tool-select" label={m.tool_kind_schema()} bind:value={newNs} options={(schema?.namespaces ?? []).map((n) => ({ value: n.name, label: n.name }))} />
                </label>
              {/if}
            </div>
            <div class="di-map" role="table" aria-label={m.tool_import_mapping()}>
              <div class="di-map-row create di-map-head" role="row">
                <span role="columnheader"></span>
                <span role="columnheader">{m.tool_import_head_file()}</span>
                <span role="columnheader"></span>
                <span role="columnheader">{m.tool_import_col_name()}</span>
                <span role="columnheader">{m.tool_import_col_type()}</span>
              </div>
              {#each columns as c, i (i)}
                <div class="di-map-row create" role="row">
                  <input role="cell" type="checkbox" aria-label={m.tool_import_include({ column: sample.columns[i]?.name ?? '' })} bind:checked={c.include} />
                  <span role="cell" class="sub">{sample.columns[i]?.name}</span>
                  <span role="cell" class="di-from">→</span>
                  <span role="cell" class="field"><input aria-label={m.tool_import_col_name()} bind:value={c.name} disabled={!c.include} spellcheck="false" /></span>
                  <span role="cell" class="field"><input aria-label={m.tool_import_col_type()} class="mono" bind:value={c.type} disabled={!c.include} spellcheck="false" /></span>
                </div>
              {/each}
            </div>
          {/if}

          <div class="di-options">
            <label class="check"><input type="checkbox" bind:checked={emptyIsNull} /> {m.tool_import_empty_null()}</label>
            <Segmented
              label={m.tool_import_on_error()}
              value={onError}
              options={[
                { value: 'stop', label: m.tool_import_stop() },
                { value: 'skip', label: m.tool_import_skip() },
              ] as const}
              onchange={(v) => (onError = v)}
            />
          </div>
          <p class="hint">{onError === 'stop' ? m.tool_import_stop_hint() : m.tool_import_skip_hint()}</p>
        {/if}
        {#if job.running}<JobBar {job} label={m.tool_import_running()} />{/if}
      {/if}
      {#if failure}<p class="test-result" role="alert">{failure}</p>{/if}
    </div>
    <footer class="dialog-foot">
      {#if production && session}<WriteLock sessionId={session.info.session_id} />{/if}
      <span class="spacer"></span>
      {#if !result?.committed && !job.running}<button class="btn ghost" type="button" onclick={close}>{result ? m.org_close() : m.form_cancel()}</button>{/if}
      {#if result?.committed}
        <button class="btn primary" type="submit">{m.org_close()}</button>
      {:else if path && sample}
        <button class="btn primary" type="submit" disabled={job.running || mapped === 0 || (mode === 'create' && !newName.trim())}>{result ? m.tool_import_again() : m.tool_import_go_count({ count: mapped })}</button>
      {/if}
    </footer>
  </form>
</dialog>
