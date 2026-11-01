<script lang="ts">
  import { ChevronRight, ClipboardCopy, FolderInput, FolderPlus, Folder, MoreHorizontal, PenLine, Pencil, Play, RefreshCw, Save, Search, SquarePlus, TextCursorInput, Trash2 } from '@lucide/svelte';
  import { copyText, openMenu, type MenuEntry } from '../lib/contextmenu.svelte';
  import { connections } from '../lib/connections.svelte';
  import { cleanFolder, folderList, groupByFolder, matchScript, visibleOn } from '../lib/library';
  import { deleteScript, library, moveScript, openScript, patchScript, renameScript } from '../lib/library.svelte';
  import { openSave } from '../lib/panel.svelte';
  import { paramsSummary } from '../lib/params';
  import { paramsIn } from '../lib/params.svelte';
  import { activeTab } from '../lib/workspace.svelte';
  import type { Script } from '../lib/api';
  import { m } from '../paraglide/messages';
  import type Editor from './Editor.svelte';
  import { suggest } from '../lib/suggest';

  let { connectionId, editor, inQuery }: { connectionId: string; editor: Editor | null; inQuery: boolean } = $props();

  let query = $state('');
  let everywhere = $state(false);
  let renaming = $state<string | null>(null);
  let moving = $state<string | null>(null);
  let closed = $state<string[]>([]);
  let failure = $state<string | null>(null);

  const scope = $derived(everywhere ? library.scripts : visibleOn(library.scripts, connectionId));
  const shown = $derived(scope.filter((s) => matchScript(s, query)));
  const groups = $derived(groupByFolder(shown));
  const folders = $derived(folderList(library.scripts));
  const canInsert = $derived(inQuery && !!editor);

  const connectionName = (id: string | null) => (id ? (connections.list.find((c) => c.id === id)?.name ?? id) : m.panel_scripts_any());

  async function guarded(work: () => Promise<unknown>) {
    failure = null;
    try {
      await work();
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    }
  }

  const typedIn = (sql: string) => {
    const t = activeTab();
    return t ? paramsIn(t.id, sql) : [];
  };

  function saveCurrent() {
    if (!editor) return;
    const sql = editor.selected() || editor.content();
    openSave(sql, connectionId, null, typedIn(sql));
  }

  function updateFromEditor(s: Script) {
    const sql = editor?.selected() || editor?.content() || s.sql;
    return patchScript(s, { sql, params: typedIn(sql) }, m.undo_script_updated());
  }

  const paramNames = (s: Script) => s.params.map((v) => (/^\d+$/.test(v.name) ? `?${v.name}` : `:${v.name}`)).join(' ');

  function menu(e: MouseEvent, s: Script) {
    const entries: MenuEntry[] = [
      { label: m.panel_scripts_run(), icon: Play, run: () => guarded(() => openScript(s, connectionId, true)) },
      { label: m.history_open_tab(), icon: SquarePlus, run: () => guarded(() => openScript(s, connectionId)) },
      { label: m.history_insert(), icon: TextCursorInput, disabled: !canInsert, run: () => editor?.insertAtCursor(s.sql) },
      'separator',
      { label: m.tab_rename(), icon: PenLine, shortcut: 'F2', run: () => (renaming = s.id) },
      {
        label: m.panel_scripts_move(),
        icon: FolderInput,
        items: [
          { label: m.panel_scripts_no_folder(), disabled: s.folder === '', run: () => guarded(() => moveScript(s, '')) },
          ...folders.filter((f) => f !== s.folder).map((f) => ({ label: f, icon: Folder, run: () => guarded(() => moveScript(s, f)) })),
          { label: m.panel_scripts_new_folder(), icon: FolderPlus, run: () => (moving = s.id) },
        ],
      },
      { label: m.panel_scripts_edit(), icon: Pencil, run: () => openSave(s.sql, connectionId, s.id) },
      { label: m.panel_scripts_update(), icon: RefreshCw, disabled: !canInsert, run: () => guarded(() => updateFromEditor(s)) },
      { label: m.menu_copy(), icon: ClipboardCopy, run: () => copyText(s.sql) },
      'separator',
      { label: m.panel_delete(), icon: Trash2, danger: true, run: () => guarded(() => deleteScript(s)) },
    ];
    openMenu(e, entries);
  }

  function rowKey(e: KeyboardEvent, s: Script) {
    if (e.key === 'F2') {
      e.preventDefault();
      renaming = s.id;
    }
    if (e.key === 'Delete') void guarded(() => deleteScript(s));
  }

  function focus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function renameKey(e: KeyboardEvent, s: Script) {
    if (e.key === 'Escape') renaming = null;
    if (e.key === 'Enter') void commitRename(e.currentTarget as HTMLInputElement, s);
  }

  async function commitRename(input: HTMLInputElement, s: Script) {
    if (renaming !== s.id) return;
    renaming = null;
    await guarded(() => renameScript(s, input.value));
  }

  function moveKey(e: KeyboardEvent, s: Script) {
    if (e.key === 'Escape') moving = null;
    if (e.key === 'Enter') void commitMove(e.currentTarget as HTMLInputElement, s);
  }

  async function commitMove(input: HTMLInputElement, s: Script) {
    if (moving !== s.id) return;
    moving = null;
    await guarded(() => moveScript(s, cleanFolder(input.value)));
  }

  const toggle = (folder: string) => (closed = closed.includes(folder) ? closed.filter((f) => f !== folder) : [...closed, folder]);
</script>

<div class="scripts">
  <div class="panel-tools">
    <label class="field">
      <Search size={13} strokeWidth={1.8} aria-hidden="true" />
      <input type="search" placeholder={m.panel_scripts_search()} aria-label={m.panel_scripts_search()} bind:value={query} />
    </label>
  </div>
  <div class="panel-actions">
    <button class="btn compact" type="button" data-tip={m.panel_scripts_save()} disabled={!canInsert} onclick={saveCurrent}>
      <Save size={13} strokeWidth={1.8} aria-hidden="true" />
      {m.panel_scripts_save_short()}
    </button>
    <button class="btn ghost compact" type="button" aria-pressed={everywhere} onclick={() => (everywhere = !everywhere)}>{m.panel_scripts_all()}</button>
  </div>

  {#if failure}<p class="error" role="alert">{failure}</p>{/if}

  {#if shown.length === 0}
    <p class="hint panel-empty">{query.trim() ? m.panel_scripts_none() : m.panel_scripts_empty()}</p>
  {:else}
    <div class="panel-scroll">
      {#each groups as g (g.folder)}
        {#if g.folder !== ''}
          <button class="group-head" type="button" aria-expanded={!closed.includes(g.folder)} onclick={() => toggle(g.folder)}>
            <ChevronRight size={12} strokeWidth={2} class="chev" aria-hidden="true" />
            <span>{g.folder}</span>
            <span class="group-n">{g.scripts.length}</span>
          </button>
        {/if}
        {#if g.folder === '' || !closed.includes(g.folder)}
          <ul class="panel-list" class:nested={g.folder !== ''}>
            {#each g.scripts as s (s.id)}
              <li class="panel-item" class:renaming={renaming === s.id}>
                {#if renaming === s.id}
                  <input class="inline-input" value={s.name} aria-label={m.tab_rename()} use:focus onkeydown={(e) => renameKey(e, s)} onblur={(e) => void commitRename(e.currentTarget, s)} />
                {:else if moving === s.id}
                  <div class="panel-row">
                    <span class="row-title">{s.name}</span>
                    <input class="inline-input" use:suggest={{ options: folders, label: m.panel_scripts_folder() }} value={s.folder} placeholder={m.panel_scripts_folder_hint()} aria-label={m.panel_scripts_folder()} use:focus onkeydown={(e) => moveKey(e, s)} onblur={(e) => void commitMove(e.currentTarget, s)} />
                  </div>
                {:else}
                  <button type="button" class="panel-row" data-tip={m.history_open_tab()} onclick={() => void guarded(() => openScript(s, connectionId))} oncontextmenu={(e) => menu(e, s)} onkeydown={(e) => rowKey(e, s)}>
                    <span class="row-title">{s.name}</span>
                    <span class="row-sub">
                      {#each s.tags as t (t)}<span class="chip">{t}</span>{/each}
                      {#if s.params.length}<span class="chip script-params-chip" data-tip={paramsSummary(s.params)}>{paramNames(s)}</span>{/if}
                      <span class="row-conn">{connectionName(s.connection_id)}</span>
                    </span>
                  </button>
                  <span class="row-actions">
                    <button type="button" class="row-act" data-tip={m.panel_scripts_run()} aria-label="{m.panel_scripts_run()}: {s.name}" onclick={() => void guarded(() => openScript(s, connectionId, true))}><Play size={13} strokeWidth={1.8} /></button>
                    <button type="button" class="row-act" data-tip={m.history_insert()} aria-label="{m.history_insert()}: {s.name}" disabled={!canInsert} onclick={() => editor?.insertAtCursor(s.sql)}><TextCursorInput size={13} strokeWidth={1.8} /></button>
                    <button type="button" class="row-act" data-tip={m.menu_label()} aria-label="{m.menu_label()}: {s.name}" onclick={(e) => menu(e, s)}><MoreHorizontal size={13} strokeWidth={1.8} /></button>
                  </span>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      {/each}
    </div>
  {/if}
</div>
