<script lang="ts">
  import { ClipboardCopy, FileDown, FileText, RefreshCw, ShieldCheck, TextCursorInput } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { api, ApiError, type SecurityReport } from '../lib/api';
  import { copyText } from '../lib/contextmenu.svelte';
  import { CATEGORY, counts, dateStamp, fixText, found, markdownReport, nothingToCheck, passed, pdfReport, SEVERITY, SEVERITY_GROUP, skipped, SKIP, subjectLine, titleOf, whatOf } from '../lib/seclab';
  import { appearance } from '../lib/appearance.svelte';
  import { getLocale } from '../paraglide/runtime';
  import { errorText } from '../lib/tools.svelte';
  import { updateErrorText } from '../lib/update.svelte';
  import { newTab, type sessionFor } from '../lib/workspace.svelte';
  import { connections } from '../lib/connections.svelte';
  import { m } from '../paraglide/messages';

  let { session }: { session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const sid = $derived(session.info.session_id);
  let report = $state.raw<SecurityReport | null>(null);
  let failure = $state<string | null>(null);
  let unsupported = $state(false);
  let running = $state(false);
  let note = $state<string | null>(null);

  async function run() {
    if (running) return;
    running = true;
    note = null;
    try {
      report = await api.securityCheck(sid);
      failure = null;
    } catch (e) {
      unsupported = e instanceof ApiError && e.error.code === 'tool' && e.error.detail.kind === 'unsupported';
      failure = errorText(e);
    } finally {
      running = false;
    }
  }

  $effect(() => {
    void sid;
    untrack(() => void run());
  });

  const c = $derived(report ? counts(report) : null);
  const list = $derived(report ? found(report) : []);
  const connection = $derived(connections.list.find((x) => x.id === session.info.connection_id)?.name ?? session.info.server.product);
  const meta = $derived({ connection, date: dateStamp() });
  const SEVERITIES = ['high', 'medium', 'low'] as const;

  async function save() {
    if (!report) return;
    const name = m.sec_file_name({ database: report.database || 'db' });
    if (await api.exportSave(name, new TextEncoder().encode(markdownReport(report, meta)))) note = m.sec_saved({ name });
  }

  async function savePdf() {
    if (!report) return;
    const name = m.sec_pdf_name({ database: report.database || 'db' });
    try {
      if (await api.reportPdf(name, pdfReport(report, meta, appearance.value.style, getLocale()))) note = m.sec_saved({ name });
    } catch (e) {
      note = errorText(e);
    }
  }

  let updating = $state(false);
  let listNote = $state<string | null>(null);

  async function updateList() {
    if (!report || updating) return;
    const before = report.versions_snapshot;
    updating = true;
    try {
      const date = await api.seclabVersionsUpdate();
      if (date > before) await run();
      listNote = date > before ? m.sec_list_updated({ date }) : m.sec_list_current({ date: before });
    } catch (e) {
      listNote = updateErrorText(e);
    } finally {
      updating = false;
    }
  }

  function copyReport() {
    if (!report) return;
    copyText(markdownReport(report, meta));
    note = m.sec_copied();
  }
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.sec_title()}</h2>
    {#if report && c}
      <p class="hint">{report.product} {report.version} · {report.database}</p>
    {:else}
      <p class="hint">{m.sec_readonly()}</p>
    {/if}
  </div>
  <span class="spacer"></span>
  {#if report}
    <button class="btn ghost" type="button" onclick={copyReport}><ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" /> {m.sec_copy_md()}</button>
    <button class="btn ghost" type="button" onclick={() => void save()}><FileDown size={13} strokeWidth={1.8} aria-hidden="true" /> {m.sec_export_md()}</button>
    <button class="btn ghost" type="button" onclick={() => void savePdf()}><FileText size={13} strokeWidth={1.8} aria-hidden="true" /> {m.sec_export_pdf()}</button>
  {/if}
  <button class="icon-btn" type="button" aria-label={m.sec_rerun()} disabled={running} onclick={() => void run()}><RefreshCw size={13} strokeWidth={1.8} class={running ? 'spin' : ''} /></button>
</header>

{#if unsupported}
  <p class="tool-empty">{m.sec_unsupported()}</p>
{:else if failure && !report}
  <p class="tool-empty err">{failure}</p>
{:else if !report}
  <p class="tool-empty">{running ? m.sec_running() : m.sec_idle()}</p>
{:else if c}
  <div class="tool-scroll sec">
    <p class="sec-readonly"><ShieldCheck size={14} strokeWidth={1.7} aria-hidden="true" /> {m.sec_readonly()}</p>
    {#if note}<p class="tool-done" role="status">{note}</p>{/if}

    <div class="sec-tally" role="group" aria-label={m.sec_title()}>
      {#each SEVERITIES as s (s)}
        <div class="sec-count {s}" class:zero={c[s] === 0}>
          <span class="sec-num">{c[s]}</span>
          <span class="sec-lbl">{SEVERITY[s]()}</span>
        </div>
      {/each}
      <div class="sec-count passed">
        <span class="sec-num">{c.passed}</span>
        <span class="sec-lbl">{m.sec_passed()}</span>
      </div>
      <div class="sec-count skipped">
        <span class="sec-num">{c.skipped}</span>
        <span class="sec-lbl">{m.sec_skipped()}</span>
      </div>
    </div>

    {#if nothingToCheck(report)}<p class="tool-note sec-flat">{m.sec_unsupported()}</p>{/if}
    {#if !list.length}<p class="tool-done big sec-flat">{m.sec_none()}</p>{/if}

    {#each SEVERITIES as s (s)}
      {@const group = list.filter((f) => f.severity === s)}
      {#if group.length}
        <h3 class="caps tool-sub sec-group {s}">{SEVERITY_GROUP[s]()} <span class="sec-n">{group.length}</span></h3>
        {#each group as f (f.id)}
          <article class="sec-card {s}" data-check={f.id}>
            <header class="sec-card-head">
              <span class="sec-sev {s}">{SEVERITY[s]()}</span>
              <h4 class="sec-name">{titleOf(f.id)}</h4>
              <span class="spacer"></span>
              <span class="sec-cat">{CATEGORY[f.category]()}</span>
            </header>
            <p class="sec-what">{whatOf(f.id)}</p>
            {#if f.subjects.length}
              <ul class="sec-subjects" aria-label={m.sec_where()}>
                {#each f.subjects as sub (sub.name + (sub.detail ?? ''))}<li class="mono">{subjectLine(f.id, sub)}</li>{/each}
              </ul>
            {/if}
            {#if f.fix}
              {@const fix = fixText(f.fix)}
              <div class="sec-fix">
                <div class="sec-fix-head">
                  <span class="caps">{m.sec_fix()}</span>
                  <span class="spacer"></span>
                  <button class="btn ghost" type="button" onclick={() => copyText(fix)}><ClipboardCopy size={13} strokeWidth={1.8} aria-hidden="true" /> {m.sec_fix_copy()}</button>
                  <button class="btn ghost" type="button" onclick={() => newTab(session.info.connection_id, fix)}><TextCursorInput size={13} strokeWidth={1.8} aria-hidden="true" /> {m.sec_fix_open()}</button>
                </div>
                <pre class="tool-output sec-sql">{fix}</pre>
              </div>
            {/if}
          </article>
        {/each}
      {/if}
    {/each}
    {#if list.length}<p class="hint sec-flat">{m.sec_fix_note()}</p>{/if}

    {#if passed(report).length}
      <h3 class="caps tool-sub">{m.sec_passed()} <span class="sec-n">{c.passed}</span></h3>
      <ul class="sec-plain ok">
        {#each passed(report) as f (f.id)}<li>{titleOf(f.id)}</li>{/each}
      </ul>
    {/if}
    {#if skipped(report).length}
      <h3 class="caps tool-sub">{m.sec_skipped()} <span class="sec-n">{c.skipped}</span></h3>
      <ul class="sec-plain">
        {#each skipped(report) as f (f.id)}<li>{titleOf(f.id)}<span class="sub">{f.reason ? SKIP[f.reason]() : ''}</span></li>{/each}
      </ul>
    {/if}
    <div class="sec-list tool-foot">
      <p class="hint">{m.sec_snapshot({ date: report.versions_snapshot })}</p>
      <button class="btn small" type="button" disabled={updating} onclick={() => void updateList()}>
        <RefreshCw size={13} strokeWidth={1.8} aria-hidden="true" />
        {updating ? m.sec_list_updating() : m.sec_list_update()}
      </button>
      {#if listNote}<span class="hint" role="status">{listNote}</span>{/if}
    </div>
  </div>
{/if}
