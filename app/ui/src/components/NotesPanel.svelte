<script lang="ts">
  import { BookText, Code2, NotebookPen, X } from '@lucide/svelte';
  import { panel, type PanelTab } from '../lib/panel.svelte';
  import type { Dialect } from '../lib/bindings';
  import { m } from '../paraglide/messages';
  import NotesTab from './NotesTab.svelte';
  import Resizer from './Resizer.svelte';
  import ScriptsTab from './ScriptsTab.svelte';
  import SnippetsTab from './SnippetsTab.svelte';
  import type Editor from './Editor.svelte';

  interface Props {
    connectionId: string;
    dialect: Dialect;
    editor: Editor | null;
    inQuery: boolean;
    onclose: () => void;
  }
  let { connectionId, dialect, editor, inQuery, onclose }: Props = $props();

  const tabs: { id: PanelTab; label: () => string; icon: typeof BookText }[] = [
    { id: 'notes', label: m.panel_notes, icon: NotebookPen },
    { id: 'scripts', label: m.panel_scripts, icon: Code2 },
    { id: 'snippets', label: m.panel_snippets, icon: BookText },
  ];

  function onkeydown(e: KeyboardEvent) {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
    if (!step) return;
    e.preventDefault();
    const at = tabs.findIndex((t) => t.id === panel.tab);
    const next = tabs[(at + step + tabs.length) % tabs.length];
    if (!next) return;
    panel.tab = next.id;
    queueMicrotask(() => document.getElementById(`panel-tab-${next.id}`)?.focus());
  }
</script>

<aside class="side-panel" aria-label={m.panel_title()}>
  <Resizer pane="panel" side="start" label={m.resize_panel()} />
  <div class="panel-head">
    <div class="panel-tabs" role="tablist" aria-label={m.panel_title()} tabindex="-1" {onkeydown}>
      {#each tabs as t (t.id)}
        <button id="panel-tab-{t.id}" type="button" role="tab" aria-selected={panel.tab === t.id} aria-controls="panel-body" tabindex={panel.tab === t.id ? 0 : -1} onclick={() => (panel.tab = t.id)}>
          <t.icon size={13} strokeWidth={1.8} aria-hidden="true" />
          {t.label()}
        </button>
      {/each}
    </div>
    <button class="icon-btn" type="button" aria-label={m.panel_close()} onclick={onclose}><X size={15} strokeWidth={1.8} /></button>
  </div>
  <div class="panel-body" id="panel-body" role="tabpanel" aria-labelledby="panel-tab-{panel.tab}">
    {#if panel.tab === 'notes'}
      <NotesTab {connectionId} />
    {:else if panel.tab === 'scripts'}
      <ScriptsTab {connectionId} {editor} {inQuery} />
    {:else}
      <SnippetsTab {dialect} {editor} {inQuery} />
    {/if}
  </div>
</aside>
