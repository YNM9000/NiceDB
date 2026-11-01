<script lang="ts">
  import { commandIcon, ICON } from '../lib/icons';
  import { ArrowDown, ArrowUp, Check, ChevronRight, CircleOff, Ellipsis, FileDown, Folder, FolderInput, FolderOpen, FolderPlus, Layers, Palette, Pencil, PenLine, Plus, Power, Bookmark, Trash2, Wrench } from '@lucide/svelte';
  import { openTool } from '../lib/tools.svelte';
  import { tick } from 'svelte';
  import type { ConnectionProfile } from '../lib/api';
  import { connections, type EditMode } from '../lib/connections.svelte';
  import { connectionUrl } from '../lib/connurl';
  import { copyText, keys, openMenu, type MenuEntry } from '../lib/contextmenu.svelte';
  import { driverOf as markOf } from '../lib/drivers';
  import { isModel } from '../lib/profile-template';
  import { MARKS, TINTS, markIcon, tintIcon } from '../lib/folder-look';
  import { canShift, createFolder, currentScope, deleteFolder, holds, moveNode, org, orgUi, shiftNode, spaces, toggleFolder, toggleMember, updateFolder } from '../lib/org.svelte';
  import { buildRows, dropTarget, moveChoices, nodeOf, sameNode, type Drop, type Folder as FolderT, type Row, type TreeNode } from '../lib/tree';
  import { activeTab, closeConnection, newTab, openConnection, sessionFor, workspace } from '../lib/workspace.svelte';
  import { m } from '../paraglide/messages';
  import { explorer } from '../lib/explorer-state.svelte';
  import { keyOf } from '../lib/explorer';
  import { play } from '../lib/sound';
  import DriverMark from './DriverMark.svelte';
  import Explorer from './Explorer.svelte';

  let { filter, onedit }: { filter: string; onedit: (p: ConnectionProfile | null, mode?: EditMode, folder?: string | null) => void } = $props();

  const rows = $derived(buildRows(org.folders, connections.list, filter, currentScope()));
  const tab = $derived(activeTab());
  const envLabel = { local: m.env_local, development: m.env_development, staging: m.env_staging, production: m.env_production };
  const driverOf = (p: ConnectionProfile) => markOf(p.target, sessionFor(p.id)?.info.edition?.flavor);

  let list = $state<HTMLDivElement>();
  let press: { node: TreeNode; label: string; x: number; y: number; id: number } | null = null;
  let drag = $state<{ node: TreeNode; label: string; x: number; y: number } | null>(null);
  let drop = $state<Drop | null>(null);
  let suppressClick = false;
  let hoverOpen: { id: string; timer: ReturnType<typeof setTimeout> } | null = null;

  const zoom = () => Number.parseFloat(getComputedStyle(document.documentElement).zoom) || 1;
  const INDEX_END = 100_000;

  function parentOf(node: TreeNode): string | null {
    if (node.kind === 'folder') return org.folders.find((f) => f.id === node.id)?.parent ?? null;
    return connections.list.find((c) => c.id === node.id)?.folder ?? null;
  }

  function kids(parent: string | null): FolderT[] {
    return org.folders.filter((f) => (f.parent ?? null) === parent);
  }

  function moveMenu(node: TreeNode): MenuEntry[] {
    const allowed = new Set(moveChoices(org.folders, node, currentScope()).filter((c) => c.allowed).map((c) => c.folder.id));
    const here = parentOf(node);
    const into = (f: FolderT) => () => moveNode(node, f.id, INDEX_END);
    const level = (parent: string | null): MenuEntry[] =>
      kids(parent).flatMap((f): MenuEntry[] => {
        if (!allowed.has(f.id)) return level(f.id);
        const icon = markIcon(f.mark) ?? Folder;
        const below = level(f.id);
        if (below.length === 0) return [{ label: f.name, icon, disabled: here === f.id, run: into(f) }];
        return [{ label: f.name, icon, items: [{ label: m.org_move_here({ name: f.name }), icon: FolderInput, disabled: here === f.id, run: into(f) }, 'separator', ...below] }];
      });
    return [{ label: m.org_move_root(), icon: Layers, disabled: here === null, run: () => moveNode(node, null, INDEX_END) }, 'separator', ...level(null)];
  }

  function spacesMenu(node: TreeNode): MenuEntry[] {
    return [
      ...spaces.list.map((w) => ({ label: w.name, icon: holds(w, node) ? Check : undefined, run: () => toggleMember(w.id, node) })),
      spaces.list.length > 0 && 'separator',
      { label: m.org_space_new_with(), icon: Plus, run: () => (orgUi.space = { id: null, preset: node }) },
    ];
  }

  const shiftItems = (node: TreeNode): MenuEntry[] => [
    { label: m.org_move_up(), icon: ArrowUp, shortcut: keys('Alt+↑'), disabled: !canShift(node, -1), run: () => shiftNode(node, -1) },
    { label: m.org_move_down(), icon: ArrowDown, shortcut: keys('Alt+↓'), disabled: !canShift(node, 1), run: () => shiftNode(node, 1) },
  ];

  const exportOf = (node: TreeNode) => () => (orgUi.exporting = node.kind === 'folder' ? { folders: [node.id], connections: [] } : { folders: [], connections: [node.id] });

  function connectionMenu(e: MouseEvent | { x: number; y: number }, c: ConnectionProfile) {
    const connected = !!sessionFor(c.id);
    const node: TreeNode = { kind: 'connection', id: c.id };
    const model = isModel(c);
    openMenu(e, [
      model ? { label: m.conn_from_model(), icon: commandIcon('conn.from_model.'), run: () => onedit(c, 'follow') } : { label: connected ? m.conn_go() : m.conn_open(), icon: ICON.connect, run: () => openConnection(c.id) },
      connected && { label: m.tab_new(), icon: ICON.newTab, run: () => newTab(c.id) },
      !model && {
        label: m.view_tools(),
        icon: Wrench,
        items: [
          { label: m.tool_activity(), run: () => openTool(c.id, 'activity') },
          { label: m.tool_space(), run: () => openTool(c.id, 'space') },
          { label: m.tool_users(), run: () => openTool(c.id, 'users') },
          { label: m.tool_compare(), run: () => openTool(c.id, 'compare') },
          { label: m.tool_backup(), run: () => openTool(c.id, 'backup') },
          { label: m.tool_transfer(), run: () => openTool(c.id, 'transfer') },
        ],
      },
      'separator',
      { label: m.conn_edit_dots(), icon: ICON.edit, run: () => onedit(c, 'edit') },
      { label: m.conn_duplicate(), icon: ICON.duplicate, run: () => onedit(c, 'copy') },
      { label: m.conn_copy_url(), icon: ICON.copyUrl, hint: m.conn_copy_url_hint(), run: () => copyText(connectionUrl(c)) },
      'separator',
      { label: m.org_move_to(), icon: FolderInput, items: moveMenu(node) },
      ...shiftItems(node),
      { label: m.org_spaces(), icon: Layers, items: spacesMenu(node) },
      { label: m.org_export_dots(), icon: FileDown, run: exportOf(node) },
      'separator',
      connected && { label: m.conn_disconnect(), icon: ICON.disconnect, run: () => closeConnection(c.id) },
      { label: m.conn_delete_dots(), icon: ICON.remove, danger: true, run: () => onedit(c, 'delete') },
    ]);
  }

  async function newSubfolder(parent: string | null) {
    const f = await createFolder(parent);
    orgUi.renaming = f.id;
  }

  function folderMenu(e: MouseEvent | { x: number; y: number }, f: FolderT) {
    const node: TreeNode = { kind: 'folder', id: f.id };
    openMenu(e, [
      { label: m.org_conn_here(), icon: Plus, run: () => onedit(null, 'edit', f.id) },
      { label: m.org_subfolder(), icon: FolderPlus, run: () => newSubfolder(f.id) },
      'separator',
      { label: m.org_rename(), icon: PenLine, shortcut: 'F2', run: () => (orgUi.renaming = f.id) },
      {
        label: m.org_tint(),
        icon: Palette,
        items: [{ label: m.org_none(), icon: CircleOff, disabled: !f.tint, run: () => updateFolder(f.id, { tint: null }) }, 'separator', ...TINTS.map((t) => ({ label: t.label(), icon: tintIcon(t.id), hint: f.tint === t.id ? '✓' : undefined, run: () => updateFolder(f.id, { tint: t.id }) }))],
      },
      {
        label: m.org_mark(),
        icon: Bookmark,
        items: [{ label: m.org_none(), icon: CircleOff, disabled: !f.mark, run: () => updateFolder(f.id, { mark: null }) }, 'separator', ...MARKS.map((x) => ({ label: x.label(), icon: x.icon, hint: f.mark === x.id ? '✓' : undefined, run: () => updateFolder(f.id, { mark: x.id }) }))],
      },
      'separator',
      { label: m.org_move_to(), icon: FolderInput, items: moveMenu(node) },
      ...shiftItems(node),
      { label: m.org_spaces(), icon: Layers, items: spacesMenu(node) },
      { label: m.org_export_dots(), icon: FileDown, run: exportOf(node) },
      'separator',
      { label: m.org_folder_delete(), icon: Trash2, danger: true, hint: m.org_folder_delete_hint(), run: () => deleteFolder(f.id) },
    ]);
  }

  function menuFromButton(e: MouseEvent, r: Row) {
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const at = { x: b.left, y: b.bottom + 2 };
    if (r.kind === 'folder') folderMenu(at, r.folder);
    else connectionMenu(at, r.conn);
  }

  function labelOf(r: Row) {
    return r.kind === 'folder' ? r.folder.name : r.conn.name;
  }

  function pointerdown(e: PointerEvent, r: Row) {
    if (e.button !== 0 || orgUi.renaming || (e.target as HTMLElement).closest('.conn-act, input')) return;
    press = { node: nodeOf(r), label: labelOf(r), x: e.clientX, y: e.clientY, id: e.pointerId };
    window.addEventListener('pointermove', pointermove);
    window.addEventListener('pointerup', pointerup);
    window.addEventListener('keydown', dragKey, true);
  }

  function target(x: number, y: number): Drop | null {
    if (!drag || !list) return null;
    const box = list.getBoundingClientRect();
    if (x < box.left || x > box.right || y < box.top || y > box.bottom) return null;
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-row]');
    if (!el || !list.contains(el)) return dropTarget(rows, org.folders, drag.node, rows.length, 0);
    const i = Number(el.dataset.row);
    const r = el.getBoundingClientRect();
    return dropTarget(rows, org.folders, drag.node, i, (y - r.top) / Math.max(1, r.height));
  }

  function openWhileHovering(d: Drop | null) {
    const row = d && d.place === 'into' && d.row >= 0 ? rows[d.row] : undefined;
    const id = row?.kind === 'folder' && !row.open ? row.folder.id : null;
    if (hoverOpen?.id === id) return;
    if (hoverOpen) clearTimeout(hoverOpen.timer);
    hoverOpen = id ? { id, timer: setTimeout(() => toggleFolder(id), 700) } : null;
  }

  function pointermove(e: PointerEvent) {
    if (!press || e.pointerId !== press.id) return;
    if (!drag) {
      if (Math.hypot(e.clientX - press.x, e.clientY - press.y) < 5) return;
      drag = { node: press.node, label: press.label, x: e.clientX, y: e.clientY };
      document.body.classList.add('tree-dragging');
    }
    drag.x = e.clientX;
    drag.y = e.clientY;
    drop = target(e.clientX, e.clientY);
    openWhileHovering(drop);
  }

  function endDrag() {
    window.removeEventListener('pointermove', pointermove);
    window.removeEventListener('pointerup', pointerup);
    window.removeEventListener('keydown', dragKey, true);
    document.body.classList.remove('tree-dragging');
    if (hoverOpen) clearTimeout(hoverOpen.timer);
    hoverOpen = null;
    press = null;
    drag = null;
    drop = null;
  }

  function pointerup(e: PointerEvent) {
    if (press && e.pointerId !== press.id) return;
    const d = drop;
    const node = drag?.node;
    if (drag) {
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
    }
    endDrag();
    if (!node || !d) return;
    play('drop');
    void moveNode(node, d.parent, d.index);
  }

  function dragKey(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !drag) return;
    e.preventDefault();
    e.stopPropagation();
    endDrag();
  }

  const connKey = (id: string) => keyOf(id, 'conn');

  function clickRow(r: Row, e?: MouseEvent) {
    if (suppressClick) return;
    if (r.kind === 'folder') {
      toggleFolder(r.folder.id);
      return;
    }
    const id = r.conn.id;
    if (isModel(r.conn)) {
      onedit(r.conn);
      return;
    }
    const fold = !!sessionFor(id) && (!!(e?.target as HTMLElement | undefined)?.closest('.conn-chev') || tab?.connectionId === id);
    if (fold) explorer.open[connKey(id)] = !explorer.open[connKey(id)];
    else explorer.open[connKey(id)] = true;
    if (!(fold && (e?.target as HTMLElement | undefined)?.closest('.conn-chev'))) void openConnection(id);
  }

  async function shiftByKey(r: Row, step: -1 | 1) {
    await shiftNode(nodeOf(r), step);
    await tick();
    const sel = r.kind === 'folder' ? `[data-folder="${r.folder.id}"] .conn-main` : `[data-conn="${r.conn.id}"] .conn-main`;
    list?.querySelector<HTMLElement>(sel)?.focus();
  }

  function rowKey(e: KeyboardEvent, r: Row) {
    if (e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
      e.preventDefault();
      void shiftByKey(r, e.key === 'ArrowUp' ? -1 : 1);
      return;
    }
    if (r.kind !== 'folder') {
      const id = r.conn.id;
      if (!sessionFor(id)) return;
      if (e.key === 'ArrowRight' && !explorer.open[connKey(id)]) explorer.open[connKey(id)] = true;
      else if (e.key === 'ArrowLeft' && explorer.open[connKey(id)]) explorer.open[connKey(id)] = false;
      else if (e.key === 'ArrowDown' && explorer.open[connKey(id)]) list?.querySelector<HTMLElement>(`[data-ex="${id}"] [tabindex="0"]`)?.focus();
      else return;
      e.preventDefault();
      return;
    }
    if (e.key === 'F2') orgUi.renaming = r.folder.id;
    else if (e.key === 'ArrowRight' && !r.open) toggleFolder(r.folder.id);
    else if (e.key === 'ArrowLeft' && r.open) toggleFolder(r.folder.id);
    else return;
    e.preventDefault();
  }

  async function commitRename(input: HTMLInputElement, f: FolderT) {
    const name = input.value.trim();
    orgUi.renaming = null;
    if (name && name !== f.name) await updateFolder(f.id, { name });
    await tick();
    list?.querySelector<HTMLElement>(`[data-folder="${f.id}"] .conn-main`)?.focus();
  }

  function renameKey(e: KeyboardEvent, f: FolderT) {
    if (e.key === 'Enter') void commitRename(e.currentTarget as HTMLInputElement, f);
    if (e.key === 'Escape') orgUi.renaming = null;
    e.stopPropagation();
  }

  function autoselect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  const dropClass = (i: number) => (drop && drop.row === i ? `drop-${drop.place}` : '');
  const dragged = (r: Row) => !!drag && sameNode(drag.node, nodeOf(r));

  function ghostStyle() {
    if (!drag) return '';
    const z = zoom();
    return `left: ${drag.x / z + 14}px; top: ${drag.y / z + 10}px;`;
  }
</script>

<div class="conn-list" class:drop-root={drop?.row === -1} class:dragging={!!drag} bind:this={list} role="list">
  {#each rows as r, i (r.kind === 'folder' ? `f:${r.folder.id}` : `c:${r.conn.id}`)}
    {#if r.kind === 'folder'}
      {@const f = r.folder}
      {@const Mark = markIcon(f.mark)}
      <div
        class="conn folder-row {dropClass(i)}"
        class:dragged={dragged(r)}
        data-row={i}
        data-folder={f.id}
        data-tint={f.tint ?? undefined}
        style:--depth={r.depth}
        role="listitem"
        oncontextmenu={(e) => folderMenu(e, f)}
        onpointerdown={(e) => pointerdown(e, r)}
      >
        {#if orgUi.renaming === f.id}
          <span class="folder-chev" aria-hidden="true"></span>
          <input class="folder-rename" value={f.name} aria-label={m.org_rename()} use:autoselect onkeydown={(e) => renameKey(e, f)} onblur={(e) => void commitRename(e.currentTarget, f)} />
        {:else}
          <button class="conn-main" type="button" aria-expanded={r.open} onclick={() => clickRow(r)} onkeydown={(e) => rowKey(e, r)}>
            <span class="folder-chev" class:open={r.open} aria-hidden="true"><ChevronRight size={13} strokeWidth={1.9} /></span>
            <span class="folder-icon" aria-hidden="true">
              {#if Mark}<Mark size={15} strokeWidth={1.7} />{:else if r.open}<FolderOpen size={15} strokeWidth={1.7} />{:else}<Folder size={15} strokeWidth={1.7} />{/if}
            </span>
            <span class="conn-name folder-name">{f.name}</span>
            <span class="folder-count" aria-label={m.org_count({ count: r.count })}>{r.count}</span>
          </button>
          <button class="icon-btn conn-act" type="button" aria-label={m.org_folder_actions({ name: f.name })} onclick={(e) => menuFromButton(e, r)}><Ellipsis size={13} strokeWidth={1.8} /></button>
        {/if}
      </div>
    {:else}
      {@const c = r.conn}
      {@const s = sessionFor(c.id)}
      {@const opening = !!workspace.opening[c.id]}
      {@const failed = workspace.openErrors[c.id]}
      <div
        class="conn {dropClass(i)}"
        class:on={tab?.connectionId === c.id}
        class:connected={!!s}
        class:prod={c.environment === 'production'}
        class:model={isModel(c)}
        class:dragged={dragged(r)}
        data-row={i}
        data-conn={c.id}
        style:--depth={r.depth}
        role="listitem"
        oncontextmenu={(e) => connectionMenu(e, c)}
        onpointerdown={(e) => pointerdown(e, r)}
      >
        <button class="conn-main" type="button" aria-expanded={s ? !!explorer.open[connKey(c.id)] : undefined} onclick={(e) => clickRow(r, e)} onkeydown={(e) => rowKey(e, r)} data-tip={failed ? `${m.conn_open_failed()}: ${failed.message}` : isModel(c) ? `${c.name} — ${m.model_open_hint()}` : c.name}>
          <span class="conn-chev" class:open={!!s && !!explorer.open[connKey(c.id)]} class:none={!s} aria-hidden="true"><ChevronRight size={13} strokeWidth={1.9} /></span>
          <span class="conn-mark"><DriverMark driver={driverOf(c)} size={16} /></span>
          <span class="conn-name">{c.name}</span>
          {#if opening}<span class="conn-state busy" aria-label={m.conn_connecting()}></span>
          {:else if failed}<span class="conn-state down" aria-label={m.conn_open_failed()}></span>
          {:else if s}<span class="conn-state up"></span>{/if}
          {#if isModel(c)}<span class="model-tag">{m.model_tag()}</span>
          {:else if c.environment === 'production' || c.environment === 'staging'}<span class="env env-{c.environment}">{envLabel[c.environment]()}</span>{/if}
        </button>
        {#if s}
          <button class="icon-btn conn-act" type="button" aria-label={m.conn_disconnect()} onclick={() => void closeConnection(c.id)}><Power size={13} strokeWidth={1.8} /></button>
        {/if}
        <button class="icon-btn conn-act" type="button" aria-label={m.conn_edit()} onclick={() => onedit(c)}><Pencil size={13} strokeWidth={1.8} /></button>
      </div>
      {#if failed}<p class="conn-error" style:--depth={r.depth}>{failed.message}</p>{/if}
      {#if s && explorer.open[connKey(c.id)] && !drag}
        <div class="ex-wrap" style:--depth={r.depth}><Explorer connectionId={c.id} /></div>
      {/if}
    {/if}
  {/each}
  {#if rows.length === 0}
    <p class="tree-none">{filter ? m.org_no_match() : m.org_space_empty()}</p>
  {/if}
</div>

{#if drag}
  <div class="tree-ghost" style={ghostStyle()} aria-hidden="true">
    {#if drag.node.kind === 'folder'}<Folder size={13} strokeWidth={1.8} />{:else}{@const id = drag.node.id}{@const c = connections.list.find((x) => x.id === id)}{#if c}<DriverMark driver={driverOf(c)} size={14} />{/if}{/if}
    <span>{drag.label}</span>
    {#if !drop}<span class="tree-ghost-no">{m.org_drop_no()}</span>{/if}
  </div>
{/if}
