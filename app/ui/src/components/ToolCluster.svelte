<script lang="ts">
  import { ClipboardCopy, RefreshCw, Search } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import type { Report, ReportSection } from '../lib/bindings';
  import { clusterTiles, sectionLabel } from '../lib/cluster';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { tag } from '../lib/format';
  import type { sessionFor } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';

  let { session }: { session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const sid = $derived(session.info.session_id);
  let report = $state.raw<Report | null>(null);
  let failure = $state<string | null>(null);
  let loading = $state(false);
  let section = $state('');
  let filter = $state('');
  let at = $state<number | null>(null);

  async function load() {
    if (loading) return;
    loading = true;
    try {
      report = await api.serverReport(sid);
      failure = null;
      at = Date.now();
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => void load());

  const sections = $derived(report?.sections ?? []);
  const current = $derived<ReportSection | null>(sections.find((s) => s.id === section) ?? sections[0] ?? null);
  const tiles = $derived(report ? clusterTiles(report, session.info.server) : []);
  const q = $derived(filter.trim().toLowerCase());
  const rows = $derived(current ? (q ? current.rows.filter((r) => r.some((c) => c.toLowerCase().includes(q))) : current.rows) : []);
  const keyed = $derived(current?.group === 'info');

  function rowMenu(e: MouseEvent, row: string[]) {
    e.preventDefault();
    openMenu(e, [
      { label: m.srv_copy_row(), icon: ClipboardCopy, run: () => copyText(row.join('\t')) },
      keyed && row[1] !== undefined && { label: m.srv_copy_value(), run: () => copyText(row[1] ?? '') },
    ]);
  }

  const tone = (v: string) => (/^(green|up|active|started|open)$/i.test(v) ? 'ok' : /^(yellow|updating|creating|initializing|relocating)$/i.test(v) ? 'warn' : /^(red|down|deleting|unassigned|close)$/i.test(v) ? 'bad' : '');
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_cluster()}</h2>
    <p class="hint">{at ? m.mon_updated({ time: new Intl.DateTimeFormat(tag(), { timeStyle: 'medium' }).format(at) }) : m.schema_loading()}</p>
  </div>
  <span class="spacer"></span>
  <button class="icon-btn" type="button" aria-label={m.schema_refresh()} data-tip={m.schema_refresh()} disabled={loading} onclick={() => void load()}><RefreshCw size={13} strokeWidth={1.8} /></button>
</header>

<div class="tool-scroll srv cluster">
  {#if failure && !report}
    <p class="tool-empty err">{failure}</p>
  {:else if !report}
    <p class="tool-empty">{m.schema_loading()}</p>
  {:else}
    <div class="mon-tiles srv-tiles">
      {#each tiles as t (t.label)}
        <section class="mon-tile" aria-label={t.label}>
          <h3 class="caps">{t.label}</h3>
          <p class="mon-value"><span class="mon-big cl-{tone(t.big)}">{t.big}</span></p>
          <p class="mon-facts">{#each t.facts as f (f)}<span>{f}</span>{/each}</p>
        </section>
      {/each}
    </div>

    <div class="srv-info">
      <nav class="srv-sections" aria-label={m.tool_cluster()}>
        {#each sections as s (s.id)}
          <button type="button" class="srv-section" aria-current={current?.id === s.id ? 'true' : undefined} onclick={() => (section = s.id)}>
            <span>{sectionLabel(s.id)}</span><span class="srv-n">{s.rows.length}</span>
          </button>
        {/each}
      </nav>
      <div class="srv-body">
        <label class="field srv-filter"><Search size={13} strokeWidth={1.8} aria-hidden="true" /><input bind:value={filter} placeholder={m.srv_filter()} aria-label={m.srv_filter()} spellcheck="false" /></label>
        {#if current && rows.length}
          <table class="tool-table srv-table cluster-table">
            {#if !keyed}
              <thead><tr>{#each current.columns as c (c)}<th>{c}</th>{/each}</tr></thead>
            {/if}
            <tbody>
              {#each rows as row, i (i)}
                <tr oncontextmenu={(e) => rowMenu(e, row)}>
                  {#each row as cell, j (j)}<td class:mono={j > 0 || !keyed} class:srv-name={keyed && j === 0}><span class="cl-{tone(cell)}">{cell}</span></td>{/each}
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <p class="mon-calm">{m.cluster_empty()}</p>
        {/if}
      </div>
    </div>
  {/if}
</div>
