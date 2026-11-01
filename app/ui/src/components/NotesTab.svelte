<script lang="ts">
  import { Search } from '@lucide/svelte';
  import { connections } from '../lib/connections.svelte';
  import { excerpt, noteMatches } from '../lib/library';
  import { library } from '../lib/library.svelte';
  import { spaces } from '../lib/org.svelte';
  import type { NoteScope, SavedNote } from '../lib/api';
  import { m } from '../paraglide/messages';
  import NoteBody from './NoteBody.svelte';
  import Segmented from './Segmented.svelte';

  let { connectionId }: { connectionId: string } = $props();

  let scope = $state<NoteScope>('connection');
  let picked = $state<{ scope: NoteScope; id: string } | null>(null);
  let mode = $state<'write' | 'preview'>('write');
  let query = $state('');

  const spaceId = $derived(spaces.active ?? 'all');
  const target = $derived(picked ?? { scope, id: scope === 'connection' ? connectionId : spaceId });
  const results = $derived(query.trim() ? library.notes.filter((n) => noteMatches(n, query)) : []);

  function nameOf(s: NoteScope, id: string) {
    if (s === 'connection') return connections.list.find((c) => c.id === id)?.name ?? id;
    return id === 'all' ? m.panel_notes_all() : (spaces.list.find((w) => w.id === id)?.name ?? id);
  }

  function open(n: SavedNote) {
    picked = { scope: n.scope, id: n.scope_id };
    scope = n.scope;
    query = '';
    mode = 'write';
  }

  function pickScope(s: NoteScope) {
    scope = s;
    picked = null;
  }
</script>

<div class="notes">
  <label class="field">
    <Search size={13} strokeWidth={1.8} aria-hidden="true" />
    <input type="search" placeholder={m.panel_notes_search()} aria-label={m.panel_notes_search()} bind:value={query} />
  </label>

  {#if query.trim()}
    {#if results.length === 0}
      <p class="hint panel-empty">{m.panel_notes_none()}</p>
    {:else}
      <ul class="panel-list">
        {#each results as n (n.scope + n.scope_id)}
          <li>
            <button type="button" class="panel-row" onclick={() => open(n)}>
              <span class="row-title">{nameOf(n.scope, n.scope_id)} <span class="row-tag">{n.scope === 'connection' ? m.panel_notes_connection() : m.panel_notes_space()}</span></span>
              <span class="row-sub">{excerpt(n.body, query)}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <div class="notes-bar">
      <Segmented
        label={m.panel_notes_scope()}
        value={picked ? picked.scope : scope}
        options={[
          { value: 'connection', label: m.panel_notes_connection() },
          { value: 'workspace', label: m.panel_notes_space() },
        ] as const}
        onchange={pickScope}
      />
      <span class="spacer"></span>
      <Segmented
        label={m.panel_notes_mode()}
        value={mode}
        options={[
          { value: 'write', label: m.panel_notes_write() },
          { value: 'preview', label: m.panel_notes_preview() },
        ] as const}
        onchange={(v) => (mode = v)}
      />
    </div>
    {#key `${target.scope}:${target.id}`}
      <NoteBody scope={target.scope} id={target.id} title={nameOf(target.scope, target.id)} {mode} />
    {/key}
  {/if}
</div>
