<script lang="ts">
  import { TOOL_ICON, type Icon } from '../lib/icons';
  import { type ToolKind } from '../lib/tools';
  import { hasTool, isSql } from '../lib/family';
  import type { sessionFor, Tab } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import ToolActivity from './ToolActivity.svelte';
  import ToolBackup from './ToolBackup.svelte';
  import ToolCompare from './ToolCompare.svelte';
  import ToolGenerate from './ToolGenerate.svelte';
  import ToolMonitor from './ToolMonitor.svelte';
  import ToolRows from './ToolRows.svelte';
  import ToolSearch from './ToolSearch.svelte';
  import ToolSeclab from './ToolSeclab.svelte';
  import ToolSizes from './ToolSizes.svelte';
  import ToolTransfer from './ToolTransfer.svelte';
  import ToolUsers from './ToolUsers.svelte';
  import ToolServer from './ToolServer.svelte';
  import ToolCluster from './ToolCluster.svelte';
  import Resizer from './Resizer.svelte';
  import EditionCard from './EditionCard.svelte';

  let { tab, session }: { tab: Tab; session: NonNullable<ReturnType<typeof sessionFor>> } = $props();


  const ALL_TOOLS: { id: ToolKind; label: string; hint: string; icon: Icon }[] = [
    { id: 'server', label: m.tool_server(), hint: m.tool_server_hint(), icon: TOOL_ICON.server },
    { id: 'cluster', label: m.tool_cluster(), hint: m.tool_cluster_hint(), icon: TOOL_ICON.cluster },
    { id: 'activity', label: m.tool_activity(), hint: m.tool_activity_hint(), icon: TOOL_ICON.activity },
    { id: 'monitor', label: m.tool_monitor(), hint: m.tool_monitor_hint(), icon: TOOL_ICON.monitor },
    { id: 'space', label: m.tool_space(), hint: m.tool_space_hint(), icon: TOOL_ICON.space },
    { id: 'users', label: m.tool_users(), hint: m.tool_users_hint(), icon: TOOL_ICON.users },
    { id: 'compare', label: m.tool_compare(), hint: m.tool_compare_hint(), icon: TOOL_ICON.compare },
    { id: 'rows', label: m.tool_rows(), hint: m.tool_rows_hint(), icon: TOOL_ICON.rows },
    { id: 'search', label: m.tool_search(), hint: m.tool_search_hint(), icon: TOOL_ICON.search },
    { id: 'generate', label: m.tool_generate(), hint: m.tool_generate_hint(), icon: TOOL_ICON.generate },
    { id: 'backup', label: m.tool_backup(), hint: m.tool_backup_hint(), icon: TOOL_ICON.backup },
    { id: 'transfer', label: m.tool_transfer(), hint: m.tool_transfer_hint(), icon: TOOL_ICON.transfer },
    { id: 'seclab', label: m.tool_seclab(), hint: m.tool_seclab_hint(), icon: TOOL_ICON.seclab },
  ];
  const TOOLS = $derived(ALL_TOOLS.filter((t) => (isSql(session.info.caps) ? t.id !== 'server' && t.id !== 'cluster' : hasTool(session.info.caps, t.id))));

  $effect(() => {
    const first = TOOLS[0];
    if (first && !TOOLS.some((t) => t.id === tab.tool)) tab.tool = first.id;
  });
</script>

<div class="tools">
  <div class="tools-side">
    <nav class="tools-nav" aria-label={m.view_tools()}>
      {#each TOOLS as t (t.id)}
        <button type="button" class="tools-item" aria-current={tab.tool === t.id ? 'page' : undefined} onclick={() => (tab.tool = t.id)}>
          <t.icon size={15} strokeWidth={1.7} aria-hidden="true" />
          <span class="tools-item-text">
            <span class="tools-item-label">{t.label}</span>
            <span class="tools-item-hint">{hasTool(session.info.caps, t.id) ? t.hint : m.tool_unavailable()}</span>
          </span>
        </button>
      {/each}
    </nav>
    {#if session.info.edition}<EditionCard edition={session.info.edition} />{/if}
    <Resizer pane="tools" side="end" label={m.resize_tools()} />
  </div>
  <section class="tools-pane" aria-label={TOOLS.find((t) => t.id === tab.tool)?.label}>
    {#key `${tab.tool}-${session.info.session_id}`}
      {#if tab.tool === 'cluster'}
        <ToolCluster {session} />
      {:else if tab.tool === 'activity'}
        <ToolActivity {session} />
      {:else if tab.tool === 'monitor'}
        <ToolMonitor {session} />
      {:else if tab.tool === 'space'}
        <ToolSizes {tab} {session} />
      {:else if tab.tool === 'users'}
        <ToolUsers {tab} {session} />
      {:else if tab.tool === 'compare'}
        <ToolCompare {tab} {session} />
      {:else if tab.tool === 'rows'}
        <ToolRows {tab} {session} />
      {:else if tab.tool === 'search'}
        <ToolSearch {tab} {session} />
      {:else if tab.tool === 'generate'}
        <ToolGenerate {session} />
      {:else if tab.tool === 'backup'}
        <ToolBackup {session} />
      {:else if tab.tool === 'server'}
        <ToolServer {session} />
      {:else if tab.tool === 'seclab'}
        <ToolSeclab {session} />
      {:else}
        <ToolTransfer {tab} {session} />
      {/if}
    {/key}
  </section>
</div>
