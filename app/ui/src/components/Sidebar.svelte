<script lang="ts">
  import { Plus, Search } from '@lucide/svelte';
  import type { ConnectionProfile } from '../lib/api';
  import { connections, type EditMode } from '../lib/connections.svelte';
  import { dbs, visibilityOf } from '../lib/dbs.svelte';
  import { databaseShown, emptyVisibility, visibleSchema } from '../lib/visibility';
  import { org } from '../lib/org.svelte';
  import { activeTab, refreshSchema, sessionOf } from '../lib/workspace.svelte';
  import { explorer } from '../lib/explorer-state.svelte';
  import { openPalette } from '../lib/palette.svelte';
  import { currentBindings } from '../lib/prefs.svelte';
  import { keys } from '../lib/contextmenu.svelte';
  import { m } from '../paraglide/messages';
  import ConnectionTree from './ConnectionTree.svelte';
  import OrgMenu from './OrgMenu.svelte';
  import DatabaseBar from './DatabaseBar.svelte';
  import Resizer from './Resizer.svelte';

  let { onedit }: { onedit: (p: ConnectionProfile | null, mode?: EditMode, folder?: string | null) => void } = $props();
  let filter = $state('');

  const tab = $derived(activeTab());
  const session = $derived(tab ? sessionOf(tab) : null);
  const vis = $derived(tab ? visibilityOf(tab.connectionId) : emptyVisibility());
  const showAll = $derived(!!tab && !!dbs.showAll[tab.connectionId]);
  const paletteHint = $derived(m.pal_sidebar_hint({ keys: keys(currentBindings().palette || 'Mod+K') }));
  const toPalette = (e: KeyboardEvent, text: string, mode: 'objects' | 'library') => {
    if (e.key !== 'Enter' || !text.trim()) return;
    e.preventDefault();
    openPalette(mode, text.trim());
  };
  const hiddenCount = $derived.by(() => {
    const schema = session?.schema;
    if (!schema) return 0;
    const spare = schema.databases.filter((d) => d !== schema.current_database && !databaseShown(vis, d)).length;
    return spare + schema.namespaces.length - visibleSchema(schema, vis).namespaces.length;
  });
</script>

<nav class="sidebar" aria-label={m.nav_connections()}>
  <div class="sidebar-head">
    <h2 class="caps">{m.nav_connections()}</h2>
    <span class="spacer"></span>
    <OrgMenu />
    <button class="icon-btn" type="button" aria-label={m.conn_new()} onclick={() => onedit(null)}><Plus size={15} strokeWidth={1.8} /></button>
  </div>
  <label class="field">
    <Search size={13} strokeWidth={1.8} aria-hidden="true" />
    <input type="search" placeholder={m.search_connections()} aria-label={m.search_connections()} data-tip={paletteHint} bind:value={filter} disabled={connections.list.length === 0} onkeydown={(e) => toPalette(e, filter, 'library')} />
  </label>

  {#if session && tab}
    <DatabaseBar {tab} schema={session.schema} onrefresh={() => void refreshSchema(tab.connectionId, tab.database)} />
    {#if hiddenCount > 0 || showAll}
      <button class="tree-showall" type="button" aria-pressed={showAll} onclick={() => (dbs.showAll[tab.connectionId] = !showAll)}>
        {showAll ? m.dbs_hide_again() : m.dbs_show_all()}
        {#if !showAll}<span>{m.dbs_hidden({ count: hiddenCount })}</span>{/if}
      </button>
    {/if}
    <label class="field ex-filter">
      <input type="search" placeholder={m.schema_filter()} aria-label={m.schema_filter()} data-tip={paletteHint} bind:value={explorer.filter} onkeydown={(e) => toPalette(e, explorer.filter, 'objects')} />
    </label>
  {/if}

  {#if connections.list.length === 0 && org.folders.length === 0}
    <p class="sidebar-empty"><span>{m.sidebar_empty()}</span></p>
  {:else}
    <ConnectionTree {filter} {onedit} />
  {/if}
  <Resizer pane="sidebar" side="end" label={m.resize_sidebar()} />
</nav>
