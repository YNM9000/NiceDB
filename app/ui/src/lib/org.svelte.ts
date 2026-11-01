import { m } from '../paraglide/messages';
import { api, type ConnectionProfile, type Folder, type TreeNode, type Workspace, type WorkspaceSession } from './api';
import { connections, loadConnections } from './connections.svelte';
import { inScope, scopeFolders, type Scope } from './tree';
import { disconnect, park, parkAll, putBack, Tab, waitingAll, workspace } from './workspace.svelte';
import { settleBeforeClosing } from './tx.svelte';
import { record } from './undo.svelte';
import { cleanWidths } from './resize';
import { designOf, restoreDesign } from './design.svelte';
import { savedParams, seedParams } from './params.svelte';

export const org = $state({ folders: [] as Folder[], loaded: false });

export const spaces = $state({
  list: [] as Workspace[],
  active: null as string | null,
  all: null as WorkspaceSession | null,
  loaded: false,
});

export const orgUi = $state({
  space: null as { id: string | null; preset: TreeNode | null; remove?: boolean; vars?: boolean } | null,
  exporting: null as { folders: string[]; connections: string[] } | null,
  importing: false,
  renaming: null as string | null,
});

export const currentSpace = () => spaces.list.find((w) => w.id === spaces.active) ?? null;

export const currentScope = (): Scope | null => {
  const w = currentSpace();
  return w ? { folders: w.folders, connections: w.connections } : null;
};

export async function loadFolders() {
  org.folders = await api.foldersList();
  org.loaded = true;
}

export async function reloadTree() {
  await Promise.all([loadFolders(), loadConnections()]);
}

export async function createFolder(parent: string | null, name: string = m.org_folder_new_name()): Promise<Folder> {
  const taken = new Set(org.folders.filter((f) => f.parent === parent).map((f) => f.name.toLowerCase()));
  let label = name;
  for (let n = 2; taken.has(label.toLowerCase()); n++) label = `${name} ${n}`;
  const folder: Folder = { id: crypto.randomUUID(), parent, name: label, tint: null, mark: null, collapsed: false };
  await api.folderSave(folder);
  const w = currentSpace();
  if (w && parent === null) await saveSpace({ ...w, folders: [...w.folders, folder.id] });
  if (parent) await updateFolder(parent, { collapsed: false });
  await loadFolders();
  return folder;
}

async function saveFolderPatch(id: string, patch: Partial<Omit<Folder, 'id' | 'parent'>>) {
  const f = org.folders.find((x) => x.id === id);
  if (!f) return;
  Object.assign(f, patch);
  await api.folderSave({ ...f, ...patch });
}

export async function updateFolder(id: string, patch: Partial<Omit<Folder, 'id' | 'parent'>>) {
  const f = org.folders.find((x) => x.id === id);
  if (!f) return;
  const keys = Object.keys(patch) as (keyof typeof patch)[];
  const before = Object.fromEntries(keys.map((k) => [k, f[k]]));
  await saveFolderPatch(id, patch);
  if (keys.every((k) => k === 'collapsed')) return;
  record({
    label: 'name' in patch ? m.undo_folder_renamed() : m.undo_folder_changed(),
    undo: () => saveFolderPatch(id, before),
    redo: () => saveFolderPatch(id, patch),
    valid: () => org.folders.some((x) => x.id === id),
  });
}

export const toggleFolder = (id: string) => {
  const f = org.folders.find((x) => x.id === id);
  if (f) void updateFolder(id, { collapsed: !f.collapsed });
};

async function deleteFolderRaw(id: string) {
  await api.folderDelete(id);
  await Promise.all([reloadTree(), loadSpaces()]);
}

export async function deleteFolder(id: string) {
  const folder = org.folders.find((f) => f.id === id);
  if (!folder) return;
  const own = $state.snapshot(folder);
  const place = siblingIds({ kind: 'folder', id });
  const index = place.ids.indexOf(id);
  const childFolders = org.folders.filter((f) => f.parent === id).map((f) => f.id);
  const childConns = connections.list.filter((c) => c.folder === id).map((c) => c.id);
  const heldBy = $state.snapshot(spaces.list).filter((w) => w.folders.includes(id));
  await deleteFolderRaw(id);
  record({
    label: m.undo_folder_deleted({ name: own.name }),
    toast: true,
    undo: async () => {
      await api.folderSave(own);
      for (const [i, c] of childFolders.entries()) await api.nodeMove({ kind: 'folder', id: c }, id, i);
      for (const [i, c] of childConns.entries()) await api.nodeMove({ kind: 'connection', id: c }, id, i);
      await api.nodeMove({ kind: 'folder', id }, own.parent, index);
      await Promise.all([reloadTree(), loadSpaces()]);
      for (const w of heldBy) await restoreSpace(w);
    },
    redo: () => deleteFolderRaw(id),
    valid: (direction) => org.folders.some((f) => f.id === id) === (direction === 'redo'),
  });
}

async function moveRaw(node: TreeNode, parent: string | null, index: number) {
  await api.nodeMove(node, parent, index);
  if (parent) {
    const f = org.folders.find((x) => x.id === parent);
    if (f?.collapsed) await saveFolderPatch(parent, { collapsed: false });
  }
  await reloadTree();
  const c = node.kind === 'connection' ? connections.list.find((x) => x.id === node.id) : undefined;
  if (c) await keepInSpace(c);
  const w = currentSpace();
  if (w && node.kind === 'folder' && !scopeFolders(currentScope(), org.folders)?.has(node.id)) await saveSpace({ ...w, folders: [...w.folders, node.id] });
}

export async function keepInSpace(c: ConnectionProfile) {
  const w = currentSpace();
  if (w && !inScope(currentScope(), org.folders, c)) await saveSpace({ ...w, connections: [...w.connections, c.id] });
}

export async function moveNode(node: TreeNode, parent: string | null, index: number) {
  const { parent: from, ids } = siblingIds(node);
  const was = ids.indexOf(node.id);
  await moveRaw(node, parent, index);
  if (from === parent && (was === index || (was === ids.length - 1 && index >= ids.length))) return;
  record({
    label: node.kind === 'folder' ? m.undo_folder_moved() : m.undo_conn_moved(),
    key: `move:${node.id}`,
    undo: () => moveRaw(node, from, was),
    redo: () => moveRaw(node, parent, index),
  });
}

function siblingIds(node: TreeNode): { parent: string | null; ids: string[] } {
  if (node.kind === 'folder') {
    const parent = org.folders.find((f) => f.id === node.id)?.parent ?? null;
    return { parent, ids: org.folders.filter((f) => (f.parent ?? null) === parent).map((f) => f.id) };
  }
  const parent = connections.list.find((c) => c.id === node.id)?.folder ?? null;
  return { parent, ids: connections.list.filter((c) => (c.folder ?? null) === parent).map((c) => c.id) };
}

export function canShift(node: TreeNode, step: -1 | 1) {
  const { ids } = siblingIds(node);
  const i = ids.indexOf(node.id);
  return i >= 0 && i + step >= 0 && i + step < ids.length;
}

export async function shiftNode(node: TreeNode, step: -1 | 1) {
  if (!canShift(node, step)) return;
  const { parent, ids } = siblingIds(node);
  await moveNode(node, parent, ids.indexOf(node.id) + step);
}

export function dropTabsOf(connectionId: string) {
  workspace.tabs = workspace.tabs.filter((t) => t.connectionId !== connectionId);
  if (workspace.active && !workspace.tabs.some((t) => t.id === workspace.active)) workspace.active = workspace.tabs[0]?.id ?? null;
}

export async function loadSpaces() {
  const all = await api.workspacesGet();
  spaces.list = all.list;
  spaces.active = all.active;
  if (!spaces.loaded) spaces.all = all.all;
  spaces.loaded = true;
  return all;
}

async function saveSpace(w: Workspace) {
  w = $state.snapshot(w);
  await api.workspaceSave(w);
  const i = spaces.list.findIndex((x) => x.id === w.id);
  spaces.list = i < 0 ? [...spaces.list, w] : spaces.list.map((x) => (x.id === w.id ? w : x));
}

export function capture(): WorkspaceSession {
  const tabs = workspace.tabs.map((t) => ({
    connection_id: t.connectionId,
    title: t.title,
    text: t.text,
    view: t.view === 'tools' ? ('query' as const) : t.view,
    table: t.data.info ? { namespace: t.data.info.namespace, name: t.data.info.name } : t.table,
    database: t.database,
    params: savedParams(t.id),
    design: designOf(t)?.saved(),
    trail: t.data.saved() ?? undefined,
  }));
  const active = workspace.tabs.findIndex((t) => t.id === workspace.active);
  return { tabs, active: active < 0 ? null : active, split: workspace.split, history_open: workspace.historyOpen, widths: { ...workspace.widths } };
}

function tabsOf(session: WorkspaceSession | null): Tab[] {
  const known = new Set(connections.list.map((c) => c.id));
  return (session?.tabs ?? [])
    .filter((t) => known.has(t.connection_id))
    .map((t) => {
      const tab = new Tab(t.connection_id, t.title, t.text);
      tab.view = t.view;
      tab.table = t.table;
      tab.database = t.database ?? null;
      if (t.params?.length) seedParams(tab.id, t.params);
      if (t.design) restoreDesign(tab, t.design);
      if (t.trail) tab.data.restore(t.trail);
      return tab;
    });
}

async function restore(session: WorkspaceSession | null) {
  for (const t of workspace.tabs) if (t.runId) await api.runCancel(t.runId).catch(() => {});
  workspace.epoch++;
  const tabs = tabsOf(session);
  const wanted = new Set(tabs.map((t) => t.connectionId));
  for (const id of Object.keys(workspace.sessions)) if (!wanted.has(id)) await disconnect(id);
  const at = session?.active ?? 0;
  workspace.tabs = tabs;
  workspace.active = (tabs[at] ?? tabs[0])?.id ?? null;
  workspace.split = session?.split ?? 0.45;
  workspace.historyOpen = session?.history_open ?? false;
  workspace.widths = cleanWidths(session?.widths);
}

export async function startSpaces() {
  const all = await loadSpaces();
  await restore(all.active ? (all.list.find((w) => w.id === all.active)?.session ?? null) : all.all);
}

let saving: ReturnType<typeof setTimeout> | undefined;
let switching = false;

async function flush() {
  clearTimeout(saving);
  if (switching) return;
  const session = capture();
  const w = currentSpace();
  if (w) w.session = session;
  else spaces.all = session;
  await api.workspaceSessionSave(spaces.active, session);
}

export function watchSession() {
  if (!spaces.loaded) return;
  for (const t of workspace.tabs) void [t.title, t.text, t.view, t.table, t.data.info, t.database];
  void [workspace.tabs.length, workspace.active, workspace.split, workspace.historyOpen, Object.entries(workspace.widths)];
  clearTimeout(saving);
  saving = setTimeout(() => void flush().catch(() => {}), 600);
}

export async function switchSpace(id: string | null) {
  if (id === spaces.active || switching) return;
  if (!(await settleBeforeClosing(waitingAll(), 'space'))) return;
  await flush().catch(() => {});
  switching = true;
  try {
    spaces.active = id;
    await api.workspaceActivate(id);
    await restore(id ? (currentSpace()?.session ?? null) : spaces.all);
  } finally {
    switching = false;
  }
}

export async function createSpace(name: string, folders: string[], conns: string[], vars: Record<string, string> = {}) {
  const w: Workspace = { id: crypto.randomUUID(), name, folders, connections: conns, session: { tabs: [], active: null, split: 0.45, history_open: false, widths: {} }, vars };
  const previous = spaces.active;
  await saveSpace(w);
  await switchSpace(w.id);
  record({
    label: m.undo_space_created({ name }),
    undo: async () => {
      if (spaces.active === w.id) await switchSpace(previous);
      await deleteSpaceRaw(w.id);
    },
    redo: async () => {
      await saveSpace(w);
      await switchSpace(w.id);
    },
  });
  return w;
}

async function restoreSpace(w: Workspace) {
  const live = spaces.list.find((x) => x.id === w.id);
  await saveSpace({ ...w, session: live?.session ?? w.session });
}

async function changeSpace(id: string, label: string, next: (w: Workspace) => Workspace) {
  const w = spaces.list.find((x) => x.id === id);
  if (!w) return;
  const before = $state.snapshot(w);
  const after = next(before);
  await restoreSpace(after);
  record({ label, undo: () => restoreSpace(before), redo: () => restoreSpace(after), valid: () => spaces.list.some((x) => x.id === id) });
}

export const editSpace = (id: string, patch: Partial<Pick<Workspace, 'name' | 'folders' | 'connections' | 'vars'>>) => changeSpace(id, m.undo_space_changed(), (w) => ({ ...w, ...patch }));

export async function duplicateSpace(id: string) {
  const w = spaces.list.find((x) => x.id === id);
  if (!w) return;
  if (w.id === spaces.active) await flush().catch(() => {});
  const copy: Workspace = { ...structuredClone($state.snapshot(w)), id: crypto.randomUUID(), name: m.org_space_copy_name({ name: w.name }) };
  await saveSpace(copy);
  record({ label: m.undo_space_created({ name: copy.name }), undo: () => deleteSpaceRaw(copy.id), redo: () => saveSpace(copy) });
}

async function deleteSpaceRaw(id: string) {
  if (spaces.active === id) await switchSpace(null);
  await api.workspaceDelete(id);
  spaces.list = spaces.list.filter((w) => w.id !== id);
}

export async function deleteSpace(id: string) {
  if (spaces.active === id) await flush().catch(() => {});
  const w = spaces.list.find((x) => x.id === id);
  if (!w) return;
  const gone = $state.snapshot(w);
  const wasActive = spaces.active === id;
  await deleteSpaceRaw(id);
  record({
    label: m.undo_space_deleted({ name: gone.name }),
    toast: true,
    undo: async () => {
      await saveSpace(gone);
      if (wasActive) await switchSpace(id);
    },
    redo: () => deleteSpaceRaw(id),
  });
}

export function holds(w: Workspace, node: TreeNode) {
  return node.kind === 'folder' ? w.folders.includes(node.id) : w.connections.includes(node.id);
}

export async function toggleMember(id: string, node: TreeNode) {
  const w = spaces.list.find((x) => x.id === id);
  if (!w) return;
  const key = node.kind === 'folder' ? 'folders' : 'connections';
  await changeSpace(id, m.undo_space_changed(), (b) => ({ ...b, [key]: b[key].includes(node.id) ? b[key].filter((x) => x !== node.id) : [...b[key], node.id] }));
}

export const CONNECTION_UNDO_MS = 30_000;

export async function removeConnection(profile: ConnectionProfile) {
  const id = profile.id;
  const own = $state.snapshot(profile);
  const place = siblingIds({ kind: 'connection', id });
  const index = place.ids.indexOf(id);
  const parked = parkAll(workspace.tabs.filter((t) => t.connectionId === id));
  const active = workspace.active;
  const epoch = workspace.epoch;
  const drop = async () => {
    await disconnect(id);
    dropTabsOf(id);
    await api.connectionTrash(id);
    await loadConnections();
  };
  await drop();
  for (const { tab } of parked) park(tab);
  record({
    label: m.undo_conn_deleted({ name: own.name }),
    toast: true,
    ttl: CONNECTION_UNDO_MS,
    undo: async () => {
      await api.connectionRestore(own);
      await moveRaw({ kind: 'connection', id }, own.folder, index);
      if (workspace.epoch === epoch) await putBack(parked, active);
    },
    redo: drop,
    expire: () => api.connectionPurge(id),
  });
}

export function spaceKeys(target: Window = window) {
  const onkey = (e: KeyboardEvent) => {
    if (!e.ctrlKey || !e.altKey || e.metaKey || e.shiftKey || e.getModifierState('AltGraph')) return;
    const n = Number(/^Digit(\d)$/.exec(e.code)?.[1] ?? NaN);
    if (!(n >= 1)) return;
    const id = n === 1 ? null : spaces.list[n - 2]?.id;
    if (id === undefined) return;
    e.preventDefault();
    void switchSpace(id);
  };
  target.addEventListener('keydown', onkey);
  return () => target.removeEventListener('keydown', onkey);
}

export const spaceSize = (w: Workspace | null) =>
  w ? connections.list.filter((c) => w.connections.includes(c.id) || (c.folder !== null && isBelow(c.folder, w.folders))).length : connections.list.length;

function isBelow(folder: string, roots: readonly string[]) {
  const by = new Map(org.folders.map((f) => [f.id, f]));
  let at = by.get(folder);
  for (let i = 0; at && i <= org.folders.length; i++) {
    if (roots.includes(at.id)) return true;
    at = at.parent ? by.get(at.parent) : undefined;
  }
  return false;
}
