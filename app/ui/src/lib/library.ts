import type { SavedNote, Script, Snippet } from './api';

export const cleanFolder = (path: string) =>
  path
    .split('/')
    .map((p) => p.trim())
    .filter(Boolean)
    .join('/');

export function parseTags(text: string): string[] {
  const seen = new Set<string>();
  for (const raw of text.split(/[,;\n]/)) {
    const t = raw.trim().replace(/^#/, '');
    if (t) seen.add(t);
  }
  return [...seen];
}

const fold = (s: string) => s.toLowerCase();

export function matchScript(s: Script, query: string): boolean {
  const q = fold(query.trim());
  if (!q) return true;
  return q
    .split(/\s+/)
    .every((word) => [s.name, s.folder, s.tags.join(' '), s.sql].some((field) => fold(field).includes(word)));
}

export function matchSnippet(s: Pick<Snippet, 'name' | 'trigger' | 'description'>, query: string): boolean {
  const q = fold(query.trim());
  return !q || [s.name, s.trigger, s.description].some((f) => fold(f).includes(q));
}

export interface ScriptGroup {
  folder: string;
  scripts: Script[];
}

export const visibleOn = (scripts: readonly Script[], connectionId: string | null) => scripts.filter((s) => s.connection_id === null || s.connection_id === connectionId);

export function groupByFolder(scripts: readonly Script[]): ScriptGroup[] {
  const by = new Map<string, Script[]>();
  for (const s of scripts) by.set(s.folder, [...(by.get(s.folder) ?? []), s]);
  return [...by.entries()]
    .sort(([a], [b]) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b)))
    .map(([folder, list]) => ({ folder, scripts: list.sort((x, y) => x.name.localeCompare(y.name)) }));
}

export const folderList = (scripts: readonly Script[]) => [...new Set(scripts.map((s) => s.folder).filter(Boolean))].sort((a, b) => a.localeCompare(b));

export function noteMatches(n: SavedNote, query: string): boolean {
  const q = fold(query.trim());
  return !q || fold(n.body).includes(q);
}

export function excerpt(body: string, query: string, width = 90): string {
  const q = fold(query.trim());
  const flat = body.replace(/\s+/g, ' ').trim();
  const at = q ? fold(flat).indexOf(q) : -1;
  if (at < 0) return flat.slice(0, width);
  const from = Math.max(0, at - 24);
  return `${from > 0 ? '…' : ''}${flat.slice(from, from + width)}`;
}

export function uniqueName(name: string, taken: readonly string[]): string {
  const set = new Set(taken.map(fold));
  if (!set.has(fold(name))) return name;
  for (let n = 2; ; n++) if (!set.has(fold(`${name} ${n}`))) return `${name} ${n}`;
}
