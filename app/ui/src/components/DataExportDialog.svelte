<script lang="ts">
  import { resultFile } from '../lib/file-names';
  import { FileDown } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { api, type DataExported } from '../lib/api';
  import type { FileFormat } from '../lib/bindings';
  import { compact, num } from '../lib/format';
  import { bytes, duration, fileName } from '../lib/tools';
  import { errorText, Job, type ExportAsk } from '../lib/tools.svelte';
  import { sessionFor } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import JobBar from './JobBar.svelte';

  let { ask, onclose }: { ask: ExportAsk; onclose: () => void } = $props();

  const FORMATS: { id: FileFormat; label: string; hint: () => string }[] = [
    { id: 'csv', label: 'CSV', hint: m.tool_fmt_csv },
    { id: 'tsv', label: 'TSV', hint: m.tool_fmt_tsv },
    { id: 'json', label: 'JSON', hint: m.tool_fmt_json_out },
    { id: 'xlsx', label: 'XLSX', hint: m.tool_fmt_xlsx_out },
    { id: 'parquet', label: 'Parquet', hint: m.tool_fmt_parquet_out },
  ];

  let format = $state<FileFormat>(untrack(() => ask.format ?? 'csv'));
  let header = $state(true);
  let done = $state.raw<(DataExported & { path: string }) | null>(null);
  let failure = $state<string | null>(null);
  const job = new Job();
  let dialog: HTMLDialogElement;

  const session = $derived(sessionFor(ask.connectionId, ask.database ?? null));
  const query = untrack(() => ask.source.kind === 'query');

  $effect(() => dialog.showModal());

  function close() {
    if (job.running) void job.cancel();
    dialog.close();
    onclose();
  }

  async function start() {
    if (done) return close();
    if (!session) return;
    failure = null;
    const path = await api.pickSave(resultFile(ask.label, format), [format], format.toUpperCase());
    if (!path) return;
    try {
      const out = await job.run((job_id, onProgress) =>
        api.dataExport({ job_id, session_id: session.info.session_id, source: ask.source, path, format, header, expected_rows: ask.expected }, onProgress),
      );
      if (!out.cancelled) done = { ...out, path };
    } catch (e) {
      failure = errorText(e);
    }
  }
</script>

<dialog class="sheet-dialog tool-dialog" bind:this={dialog} oncancel={(e) => (e.preventDefault(), close())} aria-labelledby="dx-title">
  <form
    method="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      void start();
    }}
  >
    <header class="dialog-head">
      <h2 id="dx-title" class="display">{query ? m.tool_export_result() : m.tool_export_table({ name: ask.label })}</h2>
      <p class="hint">{ask.expected !== null ? m.tool_export_rows({ count: compact(ask.expected) }) : m.tool_export_query_hint()}</p>
    </header>
    <div class="dialog-body">
      {#if done}
        <p class="tool-done big" role="status">
          <FileDown size={16} strokeWidth={1.8} aria-hidden="true" />
          {m.tool_export_done({ rows: num(done.rows ?? 0), file: fileName(done.path), size: bytes(done.bytes), time: duration(done.elapsed_ms) })}
        </p>
        {#if done.truncated}<p class="tool-warn">{m.tool_export_truncated()}</p>{/if}
        <p class="hint mono">{done.path}</p>
      {:else}
        <div class="form-field">
          <span class="caps">{m.tool_export_format()}</span>
          <div class="fmt-pick" role="radiogroup" aria-label={m.tool_export_format()}>
            {#each FORMATS as f (f.id)}
              <button type="button" role="radio" class="fmt-opt" aria-checked={format === f.id} disabled={job.running} onclick={() => (format = f.id)}>
                <b>{f.label}</b>
                <span>{f.hint()}</span>
              </button>
            {/each}
          </div>
        </div>
        {#if format === 'csv' || format === 'tsv' || format === 'xlsx'}
          <label class="check"><input type="checkbox" bind:checked={header} disabled={job.running} /> {m.tool_export_header()}</label>
        {/if}
        {#if job.running}<JobBar {job} label={m.tool_export_running()} />{/if}
        {#if failure}<p class="test-result" role="alert">{failure}</p>{/if}
      {/if}
    </div>
    <footer class="dialog-foot">
      <span class="hint">{m.tool_export_stream()}</span>
      <span class="spacer"></span>
      {#if !done && !job.running}<button class="btn ghost" type="button" onclick={close}>{m.form_cancel()}</button>{/if}
      <button class="btn primary" type="submit" disabled={job.running || !session}>{done ? m.org_close() : m.tool_export_go()}</button>
    </footer>
  </form>
</dialog>
