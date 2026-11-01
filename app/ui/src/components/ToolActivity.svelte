<script lang="ts">
  import { Ban, ClipboardCopy, Pause, Play, RefreshCw, TextCursorInput, Unplug } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { api, ApiError, type Activity } from '../lib/api';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { num } from '../lib/format';
  import { lockTree } from '../lib/monitor';
  import LockTree from './LockTree.svelte';
  import { duration } from '../lib/tools';
  import { errorText, killServerSession } from '../lib/tools.svelte';
  import { newTab, type sessionFor } from '../lib/workspace.svelte';
  import type { KillMode, ServerSession_Serialize as ServerSession } from '../lib/bindings';
  import { m } from '../paraglide/messages';

  let { session }: { session: NonNullable<ReturnType<typeof sessionFor>> } = $props();

  const sid = $derived(session.info.session_id);
  let data = $state.raw<Activity | null>(null);
  let failure = $state<string | null>(null);
  let unsupported = $state(false);
  let note = $state<string | null>(null);
  let auto = $state(true);
  let hideIdle = $state(true);
  let loading = $state(false);

  async function load() {
    if (loading) return;
    loading = true;
    try {
      data = await api.serverActivity(sid);
      failure = null;
    } catch (e) {
      unsupported = e instanceof ApiError && e.error.code === 'tool' && e.error.detail.kind === 'unsupported';
      failure = errorText(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void sid;
    untrack(() => void load());
    if (!auto || unsupported) return;
    const t = setInterval(() => {
      if (!document.hidden) void load();
    }, 3000);
    return () => clearInterval(t);
  });

  const tree = $derived(lockTree(data?.sessions ?? [], data?.locks ?? []));
  const blocking = $derived(new Set(data?.sessions.flatMap((s) => s.blocked_by) ?? []));
  const shown = $derived((data?.sessions ?? []).filter((s) => !hideIdle || s.state !== 'idle' || blocking.has(s.id)));
  const active = $derived(data?.sessions.filter((s) => s.state === 'active').length ?? 0);
  const stateLabel = (s: ServerSession) => {
    const st = (s.state ?? '').toLowerCase();
    if (st === 'active' || st === 'query' || st === 'execute') return { cls: 'run', text: m.tool_state_active() };
    if (st.startsWith('idle in transaction')) return { cls: 'wait', text: m.tool_state_idle_tx() };
    if (st === 'idle' || st === 'sleep') return { cls: 'idle', text: m.tool_state_idle() };
    return { cls: 'other', text: s.state ?? '—' };
  };

  async function kill(s: ServerSession, mode: KillMode) {
    note = null;
    const said = await killServerSession(session.info, s, mode);
    if (said === null) return;
    note = said;
    await load();
  }

  function rowMenu(e: MouseEvent, s: ServerSession) {
    openMenu(e, [
      ...(data?.can_cancel === false ? [] : [{ label: m.tool_kill_cancel(), icon: Ban, disabled: stateLabel(s).cls !== 'run', run: () => kill(s, 'cancel') }]),
      { label: m.tool_kill_close(), icon: Unplug, danger: true, run: () => kill(s, 'terminate') },
      'separator',
      { label: m.tool_copy_query(), icon: ClipboardCopy, disabled: !s.query, run: () => copyText(s.query ?? '') },
      { label: m.tree_new_query(), icon: TextCursorInput, disabled: !s.query, run: () => newTab(session.info.connection_id, s.query ?? '') },
    ]);
  }
</script>

<header class="tool-head">
  <div class="tool-title">
    <h2 class="display">{m.tool_activity()}</h2>
    {#if data}<p class="hint">{m.tool_activity_summary({ sessions: num(data.sessions.length), active: num(active), locks: num(data.locks.length) })}</p>{/if}
  </div>
  <span class="spacer"></span>
  {#if !unsupported}
    <label class="check"><input type="checkbox" bind:checked={hideIdle} /> {m.tool_hide_idle()}</label>
    <button class="btn ghost" type="button" aria-pressed={auto} onclick={() => (auto = !auto)}>
      {#if auto}<Pause size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_auto_on()}{:else}<Play size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_auto_off()}{/if}
    </button>
    <button class="icon-btn" type="button" aria-label={m.schema_refresh()} onclick={() => void load()}><RefreshCw size={13} strokeWidth={1.8} /></button>
  {/if}
</header>

{#if unsupported}
  <p class="tool-empty">{m.tool_activity_sqlite()}</p>
{:else if failure && !data}
  <p class="tool-empty err">{failure}</p>
{:else if data}
  {#if note}<p class="tool-note" role="status">{note}</p>{/if}
  {#if data.partial}<p class="tool-note">{m.tool_partial()}</p>{/if}
  <div class="tool-scroll">
    <table class="tool-table activity">
      <thead>
        <tr>
          <th class="num">{m.tool_col_pid()}</th>
          <th>{m.tool_col_user()}</th>
          <th>{m.tool_col_client()}</th>
          <th>{m.tool_col_state()}</th>
          <th class="num">{m.tool_col_duration()}</th>
          <th>{m.tool_col_query()}</th>
          <th><span class="tool-sr">{m.tool_col_actions()}</span></th>
        </tr>
      </thead>
      <tbody>
        {#each shown as s (s.id)}
          {@const st = stateLabel(s)}
          <tr class:blocked={s.blocked_by.length > 0} class:blocker={blocking.has(s.id)} oncontextmenu={(e) => rowMenu(e, s)}>
            <td class="num mono">{s.id}</td>
            <td>{s.user ?? '—'}<span class="sub">{s.database ?? ''}</span></td>
            <td>{s.application || '—'}<span class="sub">{s.client ?? ''}</span></td>
            <td>
              <span class="tool-state {st.cls}"><i aria-hidden="true"></i>{st.text}</span>
              {#if s.blocked_by.length}<span class="tool-tag warn">{m.tool_blocked_by({ ids: s.blocked_by.join(', ') })}</span>{/if}
              {#if blocking.has(s.id)}<span class="tool-tag bad">{m.tool_blocking()}</span>{/if}
            </td>
            <td class="num">{s.duration_ms === null ? '—' : duration(s.duration_ms)}</td>
            <td class="query" data-tip={s.query ?? ''}><code>{s.query ?? ''}</code>{#if s.wait}<span class="sub">{s.wait}</span>{/if}</td>
            <td class="acts">
              {#if data.can_cancel}<button class="icon-btn" type="button" data-tip={m.tool_kill_cancel()} aria-label={m.tool_kill_cancel()} disabled={st.cls !== 'run'} onclick={() => kill(s, 'cancel')}><Ban size={13} strokeWidth={1.8} /></button>{/if}
              <button class="icon-btn danger" type="button" data-tip={m.tool_kill_close()} aria-label={m.tool_kill_close()} onclick={() => kill(s, 'terminate')}><Unplug size={13} strokeWidth={1.8} /></button>
            </td>
          </tr>
        {:else}
          <tr><td colspan="7" class="tool-none">{m.tool_no_sessions()}</td></tr>
        {/each}
      </tbody>
    </table>
    {#if tree.length}
      <h3 class="caps tool-sub">{m.mon_locks()}</h3>
      <LockTree rows={tree} onKill={(s) => void kill(s, 'terminate')} onQuery={(sql) => newTab(session.info.connection_id, sql)} />
      <ul class="tool-locks">
        {#each data.locks as l, i (i)}
          <li>
            <span class="mono">{l.waiting}</span>
            <span class="tool-arrow">{m.tool_lock_waits()}</span>
            <span class="mono">{l.blocking}</span>
            {#if l.object}<span class="sub">{l.object}</span>{/if}
            {#if l.mode}<span class="sub">{l.mode}</span>{/if}
            {#if l.waited_ms !== null}<span class="sub">{duration(l.waited_ms)}</span>{/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{:else}
  <p class="tool-empty">{m.schema_loading()}</p>
{/if}
