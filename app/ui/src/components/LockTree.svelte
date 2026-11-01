<script lang="ts">
  import { ClipboardCopy, Gauge, TextCursorInput, Unplug } from '@lucide/svelte';
  import type { ServerSession_Serialize as ServerSession } from '../lib/bindings';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { lockText, oneLine, type LockRow } from '../lib/monitor';
  import { duration } from '../lib/tools';
  import { m } from '../paraglide/messages';

  let {
    rows,
    ages = new Map(),
    onKill,
    onQuery,
    onExplain = null,
  }: {
    rows: LockRow[];
    ages?: Map<string, number | null>;
    onKill: (s: ServerSession) => void;
    onQuery: (sql: string) => void;
    onExplain?: ((sql: string) => void) | null;
  } = $props();

  function menu(e: MouseEvent | { x: number; y: number }, s: ServerSession) {
    openMenu(e, [
      { label: m.tool_kill_close(), icon: Unplug, danger: true, run: () => onKill(s) },
      'separator',
      { label: m.tool_copy_query(), icon: ClipboardCopy, disabled: !s.query, run: () => copyText(s.query ?? '') },
      { label: m.tree_new_query(), icon: TextCursorInput, disabled: !s.query, run: () => onQuery(s.query ?? '') },
      onExplain && { label: m.plan_explain(), icon: Gauge, disabled: !s.query, run: () => onExplain?.(s.query ?? '') },
    ]);
  }

  function menuKey(e: KeyboardEvent, s: ServerSession) {
    if (e.key !== 'ContextMenu' && !(e.key === 'F10' && e.shiftKey)) return;
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu({ x: r.left + 16, y: r.bottom }, s);
  }

  const timeOf = (s: ServerSession) => ages.get(s.id) ?? s.duration_ms;
</script>

<ul class="lock-tree" aria-label={m.mon_locks()}>
  {#each rows as r (r.session.id)}
    {@const s = r.session}
    {@const head = r.depth === 0}
    {@const t = timeOf(s)}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <li class="lock-row" class:head tabindex="0" oncontextmenu={(e) => menu(e, s)} onkeydown={(e) => menuKey(e, s)}>
      <span class="lock-guides" aria-hidden="true">
        {#each r.rails as on, i (i)}<i class="rail" class:on></i>{/each}
        {#if !head}<i class="elbow" class:last={r.last}></i>{/if}
      </span>
      <span class="lock-who">
        <span class="mono lock-id">{s.id}</span>
        <span class="lock-user">{s.user ?? '—'}{#if s.application}<span class="sub">{` · ${s.application}`}</span>{/if}</span>
      </span>
      <span class="lock-what">
        <code data-tip={s.query ?? ''}>{oneLine(s.query) || m.tool_kill_no_query()}</code>
        <span class="sub">{[head ? m.mon_lock_holds({ count: r.waiters }) : m.mon_lock_waits(), r.on ? lockText(r.on) : s.state, t === null || t === undefined ? null : duration(t), r.on ? null : s.wait].filter(Boolean).join(' · ')}</span>
      </span>
      <span class="lock-act">
        {#if head}
          <button class="btn ghost lock-kill" type="button" onclick={() => onKill(s)}><Unplug size={13} strokeWidth={1.8} aria-hidden="true" /> {m.tool_kill_close()}</button>
        {:else}
          <button class="icon-btn danger" type="button" data-tip={m.tool_kill_close()} aria-label={m.tool_kill_close()} onclick={() => onKill(s)}><Unplug size={13} strokeWidth={1.8} /></button>
        {/if}
      </span>
    </li>
  {/each}
</ul>
