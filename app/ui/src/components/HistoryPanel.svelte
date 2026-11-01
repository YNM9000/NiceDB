<script lang="ts">
  import { ClipboardCopy, Search, SquarePlus, TextCursorInput, X } from '@lucide/svelte';
  import { copyText, openMenu } from '../lib/contextmenu.svelte';
  import { api, type HistoryEntry } from '../lib/api';
  import { num, when } from '../lib/format';
  import { m } from '../paraglide/messages';
  import Resizer from './Resizer.svelte';

  let { connectionId, oninsert, onopen, onclose }: { connectionId: string; oninsert: (sql: string) => void; onopen: (sql: string) => void; onclose: () => void } = $props();

  function entryMenu(e: MouseEvent, sql: string) {
    openMenu(e, [
      { label: m.history_insert(), icon: TextCursorInput, run: () => oninsert(sql) },
      { label: m.history_open_tab(), icon: SquarePlus, run: () => onopen(sql) },
      'separator',
      { label: m.menu_copy(), icon: ClipboardCopy, run: () => copyText(sql) },
    ]);
  }
  let query = $state('');
  let entries = $state<HistoryEntry[]>([]);

  $effect(() => {
    const q = query;
    const id = connectionId;
    const t = setTimeout(() => {
      void api.historyList(id, q, 200).then((r) => (entries = r));
    }, 120);
    return () => clearTimeout(t);
  });
</script>

<aside class="history" aria-label={m.run_history()}>
  <Resizer pane="history" side="start" label={m.resize_history()} />
  <div class="history-head">
    <h2 class="caps">{m.run_history()}</h2>
    <span class="spacer"></span>
    <button class="icon-btn" type="button" aria-label={m.history_close()} onclick={onclose}><X size={15} strokeWidth={1.8} /></button>
  </div>
  <label class="field">
    <Search size={13} strokeWidth={1.8} aria-hidden="true" />
    <input type="search" placeholder={m.history_search()} aria-label={m.history_search()} bind:value={query} />
  </label>
  {#if entries.length === 0}
    <p class="hint history-empty">{m.history_empty()}</p>
  {:else}
    <ul>
      {#each entries as e (e.id)}
        <li>
          <button type="button" class:err={!!e.error} data-tip={m.history_insert()} onclick={() => oninsert(e.sql)} oncontextmenu={(ev) => entryMenu(ev, e.sql)}>
            <code>{e.sql}</code>
            <span class="history-meta">
              {when(e.started_at ?? 0)} · {num(e.elapsed_ms ?? 0)} ms{#if e.rows !== null} · {m.results_rows({ count: num(e.rows) })}{/if}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</aside>
