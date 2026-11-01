<script lang="ts">
  import { ClipboardCopy, RefreshCw, Search } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import type { Report, ReportSection } from '../lib/bindings';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { num, tag, when } from '../lib/format';
  import { infoValue, keyspaceTotal, sectionTitle } from '../lib/server-report';
  import { duration } from '../lib/tools';
  import type { sessionFor } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import Segmented from './Segmented.svelte';

  let { session }: { session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const sid = $derived(session.info.session_id);
  let report = $state.raw<Report | null>(null);
  let failure = $state<string | null>(null);
  let loading = $state(false);
  let part = $state<'info' | 'clients' | 'slowlog' | 'memory'>('info');
  let section = $state('server');
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

  const of = (group: ReportSection['group']) => report?.sections.filter((s) => s.group === group) ?? [];
  const infos = $derived(of('info'));
  const current = $derived(infos.find((s) => s.id === section) ?? infos[0] ?? null);
  const q = $derived(filter.trim().toLowerCase());
  const rowsOf = (s: ReportSection | null) => (s ? (q ? s.rows.filter((r) => r.some((c) => c.toLowerCase().includes(q))) : s.rows) : []);
  const value = (sec: string, name: string) => (report ? infoValue(report, sec, name) : null);

  const tiles = $derived.by(() => {
    if (!report) return [];
    const hits = Number(value('stats', 'keyspace_hits') ?? 0);
    const misses = Number(value('stats', 'keyspace_misses') ?? 0);
    const used = value('memory', 'used_memory_human');
    const max = value('memory', 'maxmemory_human');
    return [
      { label: m.srv_version(), big: value('server', 'valkey_version') ?? value('server', 'redis_version') ?? '—', facts: [session.info.server.product, value('server', 'redis_mode') ?? '', value('replication', 'role') ?? ''].filter(Boolean) },
      { label: m.srv_uptime(), big: duration(Number(value('server', 'uptime_in_seconds') ?? 0) * 1000), facts: [m.srv_ops({ count: value('stats', 'instantaneous_ops_per_sec') ?? '0' })] },
      { label: m.srv_clients(), big: value('clients', 'connected_clients') ?? '—', facts: [m.srv_blocked({ count: value('clients', 'blocked_clients') ?? '0' })] },
      { label: m.srv_memory(), big: used ?? '—', facts: [max && max !== '0B' ? m.srv_of_max({ max }) : m.srv_no_max(), value('memory', 'mem_fragmentation_ratio') ? m.srv_frag({ ratio: value('memory', 'mem_fragmentation_ratio') ?? '' }) : ''].filter(Boolean) },
      { label: m.srv_keys(), big: num(keyspaceTotal(report)), facts: [hits + misses > 0 ? m.srv_hits({ pct: `${Math.round((hits / (hits + misses)) * 100)}%` }) : m.srv_no_hits()] },
    ];
  });

  function rowMenu(e: MouseEvent, row: string[]) {
    e.preventDefault();
    openMenu(e, [
      { label: m.srv_copy_row(), icon: ClipboardCopy, run: () => copyText(row.join('\t')) },
      row[1] !== undefined && { label: m.srv_copy_value(), run: () => copyText(row[1] ?? '') },
    ]);
  }

  const stamp = (s: string) => {
    const n = Number(s);
    return Number.isFinite(n) && n > 0 ? when(n * 1000) : s;
  };
  const micros = (s: string) => {
    const n = Number(s);
    return Number.isFinite(n) ? (n >= 1000 ? `${num(Math.round(n / 100) / 10)} ms` : `${num(n)} µs`) : s;
  };
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_server()}</h2>
    <p class="hint">{at ? m.mon_updated({ time: new Intl.DateTimeFormat(tag(), { timeStyle: 'medium' }).format(at) }) : m.schema_loading()}</p>
  </div>
  <span class="spacer"></span>
  <Segmented
    label={m.tool_server()}
    value={part}
    options={[
      { value: 'info', label: m.srv_part_info() },
      { value: 'clients', label: m.srv_part_clients() },
      { value: 'slowlog', label: m.srv_part_slowlog() },
      { value: 'memory', label: m.srv_part_memory() },
    ] as const}
    onchange={(v) => (part = v)}
  />
  <button class="icon-btn" type="button" aria-label={m.schema_refresh()} data-tip={m.schema_refresh()} disabled={loading} onclick={() => void load()}><RefreshCw size={13} strokeWidth={1.8} /></button>
</header>

<div class="tool-scroll srv">
  {#if failure && !report}
    <p class="tool-empty err">{failure}</p>
  {:else if !report}
    <p class="tool-empty">{m.schema_loading()}</p>
  {:else}
    <div class="mon-tiles srv-tiles">
      {#each tiles as t (t.label)}
        <section class="mon-tile" aria-label={t.label}>
          <h3 class="caps">{t.label}</h3>
          <p class="mon-value"><span class="mon-big">{t.big}</span></p>
          <p class="mon-facts">{#each t.facts as f (f)}<span>{f}</span>{/each}</p>
        </section>
      {/each}
    </div>

    {#if part === 'info'}
      <div class="srv-info">
        <nav class="srv-sections" aria-label={m.srv_part_info()}>
          {#each infos as s (s.id)}
            <button type="button" class="srv-section" aria-current={current?.id === s.id ? 'true' : undefined} onclick={() => (section = s.id)}>
              <span>{sectionTitle(s.id)}</span><span class="srv-n">{s.rows.length}</span>
            </button>
          {/each}
        </nav>
        <div class="srv-body">
          <label class="field srv-filter"><Search size={13} strokeWidth={1.8} aria-hidden="true" /><input bind:value={filter} placeholder={m.srv_filter()} aria-label={m.srv_filter()} spellcheck="false" /></label>
          {#if current}
            <table class="tool-table srv-table">
              {#if current.columns.length > 2}
                <thead><tr>{#each current.columns as c (c)}<th>{c}</th>{/each}</tr></thead>
              {/if}
              <tbody>
                {#each rowsOf(current) as row, i (i)}
                  <tr oncontextmenu={(e) => rowMenu(e, row)}>
                    {#each row as cell, j (j)}<td class:mono={j > 0 || current.columns.length > 2} class:srv-name={j === 0}>{cell}</td>{/each}
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </div>
      </div>
    {:else if part === 'clients'}
      {@const s = of('clients')[0] ?? null}
      {#if s}
        <table class="tool-table srv-table">
          <thead><tr>{#each s.columns as c (c)}<th>{c}</th>{/each}</tr></thead>
          <tbody>
            {#each s.rows as row, i (i)}
              <tr class:srv-me={row[2] === 'NiceDB'} oncontextmenu={(e) => rowMenu(e, row)}>{#each row as cell, j (j)}<td class="mono" class:num={j === 0 || j === 4 || j === 5 || j === 6}>{j === 5 || j === 6 ? duration(Number(cell) * 1000) : cell}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="mon-calm">{m.srv_no_access()}</p>
      {/if}
    {:else if part === 'slowlog'}
      {@const s = of('slowlog')[0] ?? null}
      {#if s && s.rows.length}
        <table class="tool-table srv-table srv-slow">
          <thead>
            <tr><th class="num">{m.srv_col_id()}</th><th>{m.srv_col_when()}</th><th class="num">{m.srv_col_took()}</th><th>{m.srv_col_command()}</th><th>{m.srv_col_client()}</th></tr>
          </thead>
          <tbody>
            {#each s.rows as row, i (i)}
              <tr oncontextmenu={(e) => rowMenu(e, row)}>
                <td class="num mono">{row[0]}</td>
                <td>{stamp(row[1] ?? '')}</td>
                <td class="num mono">{micros(row[2] ?? '')}</td>
                <td class="query" data-tip={row[3]}><code>{row[3]}</code></td>
                <td class="mono">{row[5] || row[4]}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="mon-calm">{s ? m.srv_slow_none() : m.srv_no_access()}</p>
      {/if}
    {:else}
      {@const stats = of('memory').find((s) => s.id === 'memory_stats')}
      {@const doctor = of('memory').find((s) => s.id === 'memory_doctor')}
      {#if doctor && doctor.rows.length}
        <div class="mon-off soft srv-doctor"><p>{doctor.rows.map((r) => r[0]).join('\n')}</p></div>
      {/if}
      {#if stats}
        <table class="tool-table srv-table">
          <tbody>
            {#each stats.rows as row, i (i)}
              <tr oncontextmenu={(e) => rowMenu(e, row)}><td class="srv-name">{row[0]}</td><td class="mono num">{row[1]}</td></tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="mon-calm">{m.srv_no_access()}</p>
      {/if}
    {/if}
  {/if}
</div>
