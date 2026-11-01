<script lang="ts">
  import { ClipboardCopy, Gauge, RefreshCw, TextCursorInput } from '@lucide/svelte';
  import { onDestroy, untrack } from 'svelte';
  import { api, type Activity, type Monitor, type SlowQueries } from '../lib/api';
  import type { GapReason, MonitorPart, ServerSession_Serialize as ServerSession, SlowQuery, SlowSource } from '../lib/bindings';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { num } from '../lib/format';
  import { ageLevel, explainable, INTERVALS, lockTree, oneLine, percent, took } from '../lib/monitor';
  import { explainQuery, monitorUi } from '../lib/monitor.svelte';
  import { registerCommands } from '../lib/palette.svelte';
  import { bytes, duration, flavorOf } from '../lib/tools';
  import { hasTool, sqlOf } from '../lib/family';
  import { errorText, killServerSession } from '../lib/tools.svelte';
  import { newTab, type sessionFor } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import LockTree from './LockTree.svelte';
  import MergeList from './MergeList.svelte';
  import LongOpList from './LongOpList.svelte';
  import Select from './Select.svelte';

  let { session }: { session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const sid = $derived(session.info.session_id);
  const cid = $derived(session.info.connection_id);
  const flavor = $derived(flavorOf(sqlOf(session.info), session.info.server.product));
  const hasSessions = $derived(hasTool(session.info.caps, 'activity'));

  let data = $state.raw<Monitor | null>(null);
  let activity = $state.raw<Activity | null>(null);
  let slow = $state.raw<SlowQueries | null>(null);
  let failure = $state<string | null>(null);
  let slowFailure = $state<string | null>(null);
  let note = $state<string | null>(null);
  let at = $state<number | null>(null);
  let loading = $state(false);
  let root: HTMLElement | undefined = $state();

  async function load() {
    if (loading) return;
    loading = true;
    try {
      const [mon, act] = await Promise.all([api.serverMonitor(sid), hasSessions ? api.serverActivity(sid).catch(() => null) : Promise.resolve(null)]);
      data = mon;
      activity = act;
      failure = null;
      at = Date.now();
    } catch (e) {
      failure = errorText(e);
    } finally {
      loading = false;
    }
  }

  async function loadSlow() {
    try {
      slow = await api.serverSlowQueries(sid);
      slowFailure = null;
    } catch (e) {
      slowFailure = errorText(e);
    }
  }

  const hidden = () => document.hidden || !root?.isConnected || root.offsetParent === null;

  $effect(() => {
    void sid;
    untrack(() => {
      void load();
      void loadSlow();
    });
  });

  $effect(() => {
    const every = monitorUi.interval;
    if (!every) return;
    const t = setInterval(() => {
      if (!hidden()) void load();
    }, every);
    const back = () => {
      if (!document.hidden) void load();
    };
    document.addEventListener('visibilitychange', back);
    return () => {
      clearInterval(t);
      document.removeEventListener('visibilitychange', back);
    };
  });

  const unregister = registerCommands([
    { id: 'monitor.refresh', label: m.mon_cmd_refresh, group: m.pal_group_tools, run: () => Promise.all([load(), loadSlow()]) },
    { id: 'monitor.pause', label: () => (monitorUi.interval ? m.mon_cmd_pause() : m.mon_cmd_resume()), group: m.pal_group_tools, run: () => (monitorUi.interval = monitorUi.interval ? 0 : 5000) },
  ]);
  onDestroy(unregister);

  const ages = $derived(new Map((data?.transactions ?? []).map((t) => [t.id, t.age_ms])));
  const tree = $derived(lockTree(activity?.sessions ?? [], activity?.locks ?? []));
  const gapOf = (part: MonitorPart) => data?.gaps.find((g) => g.part === part)?.reason ?? null;
  const oldest = $derived(data?.transactions[0] ?? null);
  const current = $derived(data?.databases.find((d) => d.name === session.info.server.database) ?? data?.databases[0] ?? null);
  const largest = $derived(Math.max(1, ...(data?.databases.map((d) => d.bytes ?? 0) ?? [])));
  const conn = $derived(data?.connections ?? null);
  const fileDb = $derived(gapOf('connections')?.kind === 'file_database');

  function why(r: GapReason): string {
    switch (r.kind) {
      case 'file_database':
        return m.mon_gap_file();
      case 'not_exposed':
        return m.mon_gap_not_exposed();
      case 'permission':
        return m.mon_gap_permission({ need: r.need });
      case 'extension_missing':
        return m.mon_gap_ext_missing({ name: r.name });
      case 'extension_not_loaded':
        return m.mon_gap_ext_not_loaded({ name: r.name });
      case 'feature_off':
        return m.mon_gap_feature_off({ name: r.name });
      case 'failed':
        return m.mon_gap_failed({ message: r.message });
    }
  }

  function howTo(r: GapReason): string | null {
    if (r.kind === 'extension_missing' && flavor === 'postgres') return m.mon_howto_pgss();
    if (r.kind === 'extension_not_loaded') return m.mon_howto_pgss_preload();
    if (r.kind === 'feature_off' && r.name === 'performance_schema') return m.mon_howto_perfschema();
    if (r.kind === 'feature_off' && r.name === 'Query Store') return m.mon_howto_querystore();
    return null;
  }

  const SOURCE: Record<SlowSource, string> = {
    pg_stat_statements: 'pg_stat_statements',
    performance_schema: 'performance_schema',
    query_store: 'Query Store',
    plan_cache: m.mon_source_plan_cache(),
    statement_statistics: 'crdb_internal.statement_statistics',
    query_log: 'system.query_log',
    statements_summary: 'STATEMENTS_SUMMARY',
  };

  async function kill(s: ServerSession) {
    note = null;
    const said = await killServerSession(session.info, s, 'terminate');
    if (said === null) return;
    note = said;
    await load();
  }

  const canExplain = $derived(explainable(flavor));
  const explainText = (q: SlowQuery) => q.sample ?? q.text;

  function slowMenu(e: MouseEvent, q: SlowQuery) {
    openMenu(e, [
      { label: m.plan_explain(), icon: Gauge, disabled: !canExplain, run: () => explainQuery(cid, explainText(q), flavor) },
      { label: m.tree_new_query(), icon: TextCursorInput, run: () => newTab(cid, explainText(q)) },
      { label: m.tool_copy_query(), icon: ClipboardCopy, run: () => copyText(q.text) },
    ]);
  }

  const clock = (t: number) => new Date(t).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  const intervalLabel = (ms: number) => (ms ? m.mon_every({ time: duration(ms) }) : m.mon_paused());
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_monitor()}</h2>
    <p class="hint">{at ? m.mon_updated({ time: clock(at) }) : m.schema_loading()}</p>
  </div>
  <span class="spacer"></span>
  <Select class="mon-every" label={m.mon_interval()} bind:value={monitorUi.interval} options={INTERVALS.map((ms) => ({ value: ms, label: intervalLabel(ms) }))} />
  <button class="icon-btn" type="button" aria-label={m.schema_refresh()} data-tip={m.schema_refresh()} onclick={() => void Promise.all([load(), loadSlow()])}><RefreshCw size={13} strokeWidth={1.8} /></button>
</header>

<div class="tool-scroll mon" bind:this={root}>
  {#if failure && !data}
    <p class="tool-empty err">{failure}</p>
  {:else if data}
    {#if note}<p class="tool-note mon-flush" role="status">{note}</p>{/if}
    <div class="mon-tiles">
      {#if !fileDb}
      <section class="mon-tile" aria-label={m.mon_connections()}>
        <h3 class="caps">{m.mon_connections()}</h3>
        {#if conn}
          <p class="mon-value"><span class="mon-big">{conn.total === null ? '—' : num(conn.total)}</span>{#if conn.max !== null}<span class="mon-of">{m.mon_of_max({ max: num(conn.max) })}</span>{/if}</p>
          <p class="mon-facts">
            {#if conn.active !== null}<span>{m.mon_active({ count: num(conn.active) })}</span>{/if}
            {#if conn.idle_in_transaction !== null}<span class:warn={conn.idle_in_transaction > 0}>{m.mon_idle_tx({ count: num(conn.idle_in_transaction) })}</span>{/if}
            {#if conn.waiting !== null}<span class:bad={conn.waiting > 0}>{m.mon_waiting({ count: num(conn.waiting) })}</span>{/if}
          </p>
        {:else}
          {@const g = gapOf('connections')}
          <p class="mon-gap">{g ? why(g) : '—'}</p>
        {/if}
      </section>
      {#if flavor === 'clickhouse'}
      {@const merges = data.merges ?? []}
      <section class="mon-tile" aria-label={m.mon_merges()}>
        <h3 class="caps">{m.mon_merges()}</h3>
        <p class="mon-value"><span class="mon-big" class:quiet={!merges.length}>{num(merges.length)}</span></p>
        <p class="mon-facts"><span>{m.mon_mutations({ count: num(merges.filter((g) => g.mutation).length) })}</span></p>
      </section>
      {:else}
      <section class="mon-tile" aria-label={m.mon_oldest_tx()}>
        <h3 class="caps">{m.mon_oldest_tx()}</h3>
        {#if oldest}
          <p class="mon-value"><span class="mon-big {ageLevel(oldest.age_ms)}">{duration(oldest.age_ms)}</span></p>
          <p class="mon-facts"><span class="mono">{oldest.id}</span><span>{oldest.user ?? '—'}</span><span>{m.mon_open_tx({ count: num(data.transactions.length) })}</span></p>
        {:else if gapOf('transactions')}
          {@const g = gapOf('transactions')}
          <p class="mon-gap">{g ? why(g) : ''}</p>
        {:else}
          <p class="mon-value"><span class="mon-big quiet">—</span></p>
          <p class="mon-facts"><span>{m.mon_no_tx()}</span></p>
        {/if}
      </section>
      {/if}
      {/if}
      <section class="mon-tile" aria-label={m.mon_size()}>
        <h3 class="caps">{m.mon_size()}</h3>
        {#if current}
          <p class="mon-value"><span class="mon-big">{bytes(current.bytes)}</span></p>
          <p class="mon-facts"><span class="mono">{current.name}</span>{#if data.databases.length > 1}<span>{m.mon_databases({ count: num(data.databases.length) })}</span>{/if}</p>
        {:else}
          {@const g = gapOf('databases')}
          <p class="mon-gap">{g ? why(g) : '—'}</p>
        {/if}
      </section>
      {#if fileDb}
        <div class="mon-off soft mon-file"><p>{m.mon_file_note()}</p></div>
      {:else}
      <section class="mon-tile" aria-label={m.mon_cache()}>
        <h3 class="caps">{m.mon_cache()}</h3>
        {#if data.cache_hit !== null}
          <p class="mon-value"><span class="mon-big" class:warn={data.cache_hit < 0.9}>{percent(data.cache_hit)}</span></p>
          <p class="mon-facts"><span>{m.mon_cache_hint()}</span></p>
        {:else}
          {@const g = gapOf('cache_hit')}
          <p class="mon-gap">{g ? why(g) : '—'}</p>
        {/if}
      </section>
      {/if}
    </div>

    {#if !fileDb}
    {#if flavor === 'clickhouse'}
    <h3 class="caps tool-sub">{m.mon_merges_now()}</h3>
    <MergeList merges={data.merges ?? []} />
    {:else}
    <div class="tool-subhead">
      <h3 class="caps tool-sub">{m.mon_locks()}</h3>
      {#if tree.length}<span class="tool-tag bad">{m.mon_lock_count({ count: num(tree.filter((r) => r.depth > 0).length) })}</span>{/if}
    </div>
    {#if tree.length}
      <LockTree rows={tree} {ages} onKill={(s) => void kill(s)} onQuery={(sql) => newTab(cid, sql)} onExplain={canExplain ? (sql) => explainQuery(cid, sql, flavor) : null} />
    {:else}
      <p class="mon-calm">{activity?.partial ? m.tool_partial() : m.mon_no_locks()}</p>
    {/if}
    {/if}

    {#if data.transactions.length}
      <h3 class="caps tool-sub">{m.mon_transactions()}</h3>
      <table class="tool-table mon-tx">
        <thead>
          <tr>
            <th class="num">{m.tool_col_pid()}</th>
            <th>{m.tool_col_user()}</th>
            <th class="num">{m.mon_col_open_for()}</th>
            <th>{m.tool_col_state()}</th>
            <th>{m.tool_col_query()}</th>
          </tr>
        </thead>
        <tbody>
          {#each data.transactions as t (t.id)}
            <tr class={ageLevel(t.age_ms)}>
              <td class="num mono">{t.id}</td>
              <td>{t.user ?? '—'}<span class="sub">{t.database ?? ''}</span></td>
              <td class="num"><span class="mon-age">{duration(t.age_ms)}</span></td>
              <td>{t.state ?? '—'}</td>
              <td class="query" data-tip={t.query ?? ''}><code>{oneLine(t.query)}</code></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    {#if flavor === 'oracle'}
      <h3 class="caps tool-sub">{m.mon_long_ops()}</h3>
      <LongOpList ops={data.long_ops ?? []} />
    {/if}

    {#if data.databases.length > 1}
      <h3 class="caps tool-sub">{m.mon_by_database()}</h3>
      <table class="tool-table mon-dbs">
        <tbody>
          {#each data.databases as d (d.name)}
            <tr class:current={d.name === current?.name}>
              <td class="mono">{d.name}</td>
              <td class="num">{bytes(d.bytes)}</td>
              <td class="bar-col"><span class="tool-bar"><i style:width={`${((d.bytes ?? 0) / largest) * 100}%`}></i></span></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    <div class="tool-subhead">
      <h3 class="caps tool-sub">{m.mon_slow()}</h3>
      {#if slow?.source}<span class="hint">{m.mon_slow_from({ source: SOURCE[slow.source] })}</span>{/if}
    </div>
    {#if slowFailure}
      <p class="mon-gap err">{slowFailure}</p>
    {:else if slow}
      {#if slow.gap}
        {@const how = howTo(slow.gap)}
        <div class="mon-off" class:soft={!!slow.source}>
          <p>{why(slow.gap)}</p>
          {#if how}<p class="hint">{how}</p>{/if}
          {#if slow.source === 'plan_cache'}<p class="hint">{m.mon_plan_cache_note()}</p>{/if}
        </div>
      {/if}
      {#if slow.queries.length}
        <table class="tool-table mon-slow">
          <thead>
            <tr>
              <th>{m.tool_col_query()}</th>
              <th class="num">{m.mon_col_calls()}</th>
              <th class="num">{m.mon_col_total()}</th>
              <th class="num">{m.mon_col_mean()}</th>
              <th class="num">{m.mon_col_rows()}</th>
              <th><span class="tool-sr">{m.tool_col_actions()}</span></th>
            </tr>
          </thead>
          <tbody>
            {#each slow.queries as q, i (i)}
              <tr oncontextmenu={(e) => slowMenu(e, q)}>
                <td class="query" data-tip={q.text}><code>{oneLine(q.text)}</code></td>
                <td class="num">{q.calls === null ? '—' : num(q.calls)}</td>
                <td class="num">{took(q.total_ms)}</td>
                <td class="num">{took(q.mean_ms)}</td>
                <td class="num">{q.rows === null ? '—' : num(Math.round(q.rows))}</td>
                <td class="acts">
                  <button class="btn ghost mon-explain" type="button" disabled={!canExplain} data-tip={canExplain ? m.mon_explain_hint() : m.mon_explain_none()} onclick={() => explainQuery(cid, explainText(q), flavor)}>
                    <Gauge size={13} strokeWidth={1.8} aria-hidden="true" />
                    {m.plan_explain()}
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else if slow.source}
        <p class="mon-calm">{m.mon_slow_none()}</p>
      {/if}
    {:else}
      <p class="mon-calm">{m.schema_loading()}</p>
    {/if}
    {/if}
  {:else}
    <p class="tool-empty">{m.schema_loading()}</p>
  {/if}
</div>
