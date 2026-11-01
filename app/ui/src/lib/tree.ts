import type { ConnectionProfile } from './api';
import type { Folder_Serialize as Folder, TreeNode } from './bindings';
import { whereOf } from './drivers';

export type { Folder, TreeNode };

export interface Scope {
  folders: readonly string[];
  connections: readonly string[];
}

export type Row =
  | { kind: 'folder'; folder: Folder; depth: number; count: number; open: boolean }
  | { kind: 'connection'; conn: ConnectionProfile; depth: number };

export type Place = 'before' | 'after' | 'into';

export interface Drop {
  parent: string | null;
  index: number;
  row: number;
  place: Place;
}

export const nodeOf = (r: Row): TreeNode => (r.kind === 'folder' ? { kind: 'folder', id: r.folder.id } : { kind: 'connection', id: r.conn.id });

export const sameNode = (a: TreeNode, b: TreeNode) => a.kind === b.kind && a.id === b.id;

function children(folders: readonly Folder[]) {
  const map = new Map<string | null, Folder[]>();
  const known = new Set(folders.map((f) => f.id));
  for (const f of folders) {
    const p = f.parent && known.has(f.parent) ? f.parent : null;
    const list = map.get(p) ?? [];
    list.push(f);
    map.set(p, list);
  }
  return map;
}

export function subtree(folders: readonly Folder[], id: string): Set<string> {
  const kids = children(folders);
  const out = new Set<string>([id]);
  const stack = [id];
  while (stack.length) {
    for (const k of kids.get(stack.pop() ?? null) ?? []) {
      if (!out.has(k.id)) {
        out.add(k.id);
        stack.push(k.id);
      }
    }
  }
  return out;
}

export function ancestors(folders: readonly Folder[], id: string | null): string[] {
  const by = new Map(folders.map((f) => [f.id, f]));
  const out: string[] = [];
  let at = id ? by.get(id) : undefined;
  while (at && out.length <= folders.length) {
    out.unshift(at.id);
    at = at.parent ? by.get(at.parent) : undefined;
  }
  return out;
}

export function inScope(scope: Scope | null, folders: readonly Folder[], c: ConnectionProfile): boolean {
  if (!scope) return true;
  if (scope.connections.includes(c.id)) return true;
  return ancestors(folders, c.folder).some((f) => scope.folders.includes(f));
}

function haystack(c: ConnectionProfile, path: string) {
  const t = c.target;
  return `${c.name} ${path} ${whereOf(t)}`.toLowerCase();
}

export function buildRows(folders: readonly Folder[], connections: readonly ConnectionProfile[], filter = '', scope: Scope | null = null): Row[] {
  const q = filter.trim().toLowerCase();
  const kids = children(folders);
  const byId = new Map(folders.map((f) => [f.id, f]));
  const known = new Set(folders.map((f) => f.id));
  const pathOf = (id: string | null) => ancestors(folders, id).map((f) => byId.get(f)?.name ?? '').join(' ');
  const scopedFolders = scope ? new Set(scope.folders.flatMap((f) => [...subtree(folders, f)])) : null;
  const conns = new Map<string | null, ConnectionProfile[]>();
  for (const c of connections) {
    if (!inScope(scope, folders, c)) continue;
    if (q && !haystack(c, pathOf(c.folder)).includes(q)) continue;
    const p = c.folder && known.has(c.folder) ? c.folder : null;
    const list = conns.get(p) ?? [];
    list.push(c);
    conns.set(p, list);
  }
  const count = new Map<string, number>();
  const tally = (id: string): number => {
    const own = conns.get(id)?.length ?? 0;
    const total = own + (kids.get(id) ?? []).reduce((n, k) => n + tally(k.id), 0);
    count.set(id, total);
    return total;
  };
  for (const f of kids.get(null) ?? []) tally(f.id);
  const shown = (f: Folder) => {
    if (q || scope) return (count.get(f.id) ?? 0) > 0 || (!q && !!scopedFolders?.has(f.id));
    return true;
  };
  const rows: Row[] = [];
  const walk = (parent: string | null, depth: number) => {
    for (const f of kids.get(parent) ?? []) {
      if (!shown(f)) continue;
      const open = !!q || !f.collapsed;
      rows.push({ kind: 'folder', folder: f, depth, count: count.get(f.id) ?? 0, open });
      if (open) walk(f.id, depth + 1);
    }
    for (const c of conns.get(parent) ?? []) rows.push({ kind: 'connection', conn: c, depth });
  };
  walk(null, 0);
  return rows;
}

const parentOf = (r: Row) => (r.kind === 'folder' ? r.folder.parent : r.conn.folder) ?? null;

function indexAt(rows: readonly Row[], dragged: TreeNode, parent: string | null, upTo: number): number {
  let n = 0;
  for (let i = 0; i < upTo; i++) {
    const r = rows[i];
    if (!r || r.kind !== dragged.kind || parentOf(r) !== parent || sameNode(nodeOf(r), dragged)) continue;
    n++;
  }
  return n;
}

function siblingsCount(rows: readonly Row[], folders: readonly Folder[], dragged: TreeNode, parent: string | null): number {
  if (dragged.kind === 'folder') return folders.filter((f) => (f.parent ?? null) === parent && f.id !== dragged.id).length;
  return rows.filter((r) => r.kind === 'connection' && parentOf(r) === parent && r.conn.id !== dragged.id).length;
}

export function dropTarget(rows: readonly Row[], folders: readonly Folder[], dragged: TreeNode, at: number, ratio: number): Drop | null {
  const blocked = dragged.kind === 'folder' ? subtree(folders, dragged.id) : new Set<string>();
  const into = (folder: string | null, row: number): Drop | null => {
    if (folder && blocked.has(folder)) return null;
    return { parent: folder, index: siblingsCount(rows, folders, dragged, folder), row, place: 'into' };
  };
  const r = rows[at];
  if (!r) return into(null, -1);
  const parent = parentOf(r);
  if (r.kind === 'folder') {
    if (blocked.has(r.folder.id)) return null;
    if (dragged.kind === 'folder' && (ratio < 0.25 || (ratio > 0.75 && !r.open))) {
      const place: Place = ratio < 0.25 ? 'before' : 'after';
      const upTo = place === 'before' ? at : at + 1;
      return { parent, index: indexAt(rows, dragged, parent, upTo), row: at, place };
    }
    return into(r.folder.id, at);
  }
  if (dragged.kind === 'folder') {
    const host = parent === null ? -1 : rows.findIndex((x) => x.kind === 'folder' && x.folder.id === parent);
    return into(parent, host);
  }
  if (sameNode(nodeOf(r), dragged)) return null;
  const place: Place = ratio < 0.5 ? 'before' : 'after';
  return { parent, index: indexAt(rows, dragged, parent, place === 'before' ? at : at + 1), row: at, place };
}

export interface FolderChoice {
  folder: Folder;
  depth: number;
  allowed: boolean;
}

export function scopeFolders(scope: Scope | null, folders: readonly Folder[]): Set<string> | null {
  if (!scope) return null;
  const out = new Set<string>();
  for (const id of scope.folders) for (const f of subtree(folders, id)) out.add(f);
  return out;
}

export function moveChoices(folders: readonly Folder[], node: TreeNode, scope: Scope | null = null): FolderChoice[] {
  const blocked = node.kind === 'folder' ? subtree(folders, node.id) : new Set<string>();
  const reach = scopeFolders(scope, folders);
  const kids = children(folders);
  const out: FolderChoice[] = [];
  const walk = (parent: string | null, depth: number) => {
    for (const f of kids.get(parent) ?? []) {
      const shown = !reach || reach.has(f.id);
      if (shown) out.push({ folder: f, depth, allowed: !blocked.has(f.id) });
      walk(f.id, shown ? depth + 1 : depth);
    }
  };
  walk(null, 0);
  return out;
}

export function folderPath(folders: readonly Folder[], id: string | null): string[] {
  const by = new Map(folders.map((f) => [f.id, f.name]));
  return ancestors(folders, id).map((f) => by.get(f) ?? '');
}

export function covered(folders: readonly Folder[], connections: readonly ConnectionProfile[], picked: readonly string[]) {
  const below = new Set<string>();
  for (const id of picked) for (const f of subtree(folders, id)) if (f !== id) below.add(f);
  const all = new Set([...picked, ...below]);
  return { folders: below, connections: new Set(connections.filter((c) => c.folder !== null && all.has(c.folder)).map((c) => c.id)) };
}

export function pickedCount(folders: readonly Folder[], connections: readonly ConnectionProfile[], sel: Scope): number {
  const inside = covered(folders, connections, sel.folders).connections;
  return connections.filter((c) => inside.has(c.id) || sel.connections.includes(c.id)).length;
}
