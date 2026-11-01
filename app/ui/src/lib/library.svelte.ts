import { m } from '../paraglide/messages';
import { api, type NoteScope, type SavedNote, type Script, type Snippet } from './api';
import { cleanFolder } from './library';
import { record } from './undo.svelte';
import { seedParams } from './params.svelte';
import { activeTab, ensureSession, newTab, run as runTab } from './workspace.svelte';

export const library = $state({ scripts: [] as Script[], snippets: [] as Snippet[], notes: [] as SavedNote[], loaded: false });

export async function loadLibrary() {
  const [scripts, snippets, notes] = await Promise.all([api.scriptsList(), api.snippetsList(), api.notesList()]);
  Object.assign(library, { scripts, snippets, notes, loaded: true });
}

const putScript = (s: Script) => (library.scripts = [...library.scripts.filter((x) => x.id !== s.id), s]);
const dropScript = (id: string) => (library.scripts = library.scripts.filter((x) => x.id !== id));
const hasScript = (id: string) => library.scripts.some((s) => s.id === id);

export type ScriptDraft = Pick<Script, 'name' | 'folder' | 'tags' | 'connection_id' | 'sql'> & { params?: Script['params'] };

export async function createScript(draft: ScriptDraft): Promise<Script> {
  const saved = await api.scriptSave({ ...draft, params: draft.params ?? [], folder: cleanFolder(draft.folder), id: crypto.randomUUID(), created_at: 0, updated_at: 0 });
  putScript(saved);
  return saved;
}

async function restoreScript(s: Script) {
  putScript(await api.scriptSave(s));
}

export async function patchScript(current: Script, patch: Partial<ScriptDraft>, label: string) {
  const before = $state.snapshot(current) as Script;
  const next = { ...before, ...patch, folder: cleanFolder(patch.folder ?? before.folder) };
  const saved = await api.scriptSave(next);
  putScript(saved);
  record({
    label,
    key: `script:${before.id}`,
    undo: () => restoreScript(before),
    redo: () => restoreScript(saved),
    valid: () => hasScript(before.id),
  });
  return saved;
}

export const renameScript = (s: Script, name: string) => (name.trim() && name.trim() !== s.name ? patchScript(s, { name: name.trim() }, m.undo_script_renamed()) : Promise.resolve(s));

export const moveScript = (s: Script, folder: string) => (cleanFolder(folder) !== s.folder ? patchScript(s, { folder }, m.undo_script_moved()) : Promise.resolve(s));

export async function deleteScript(s: Script) {
  const own = $state.snapshot(s) as Script;
  await api.scriptDelete(own.id);
  dropScript(own.id);
  record({
    label: m.undo_script_deleted({ name: own.name }),
    toast: true,
    undo: () => restoreScript(own),
    redo: async () => {
      await api.scriptDelete(own.id);
      dropScript(own.id);
    },
    valid: (direction) => hasScript(own.id) === (direction === 'redo'),
  });
}

export async function openScript(s: Script, fallback: string | null, run = false) {
  const id = s.connection_id ?? fallback;
  const here = activeTab();
  if (!id || !(await ensureSession(id, here?.connectionId === id ? here.database : null))) return false;
  const tab = newTab(id, s.sql);
  seedParams(tab.id, s.params);
  if (run) void runTab(tab, { kind: 'script' });
  return true;
}

const putSnippet = (s: Snippet) => (library.snippets = [...library.snippets.filter((x) => x.id !== s.id), s]);
const hasSnippet = (id: string) => library.snippets.some((s) => s.id === id);

async function restoreSnippet(s: Snippet) {
  await api.snippetSave(s);
  putSnippet(s);
}

export async function saveSnippet(s: Snippet) {
  const before = library.snippets.find((x) => x.id === s.id);
  const own = $state.snapshot(before) as Snippet | undefined;
  await restoreSnippet(s);
  if (own) record({ label: m.undo_snippet_edited(), key: `snippet:${s.id}`, undo: () => restoreSnippet(own), redo: () => restoreSnippet(s), valid: () => hasSnippet(s.id) });
}

export async function deleteSnippet(s: Snippet) {
  const own = $state.snapshot(s) as Snippet;
  const remove = async () => {
    await api.snippetDelete(own.id);
    library.snippets = library.snippets.filter((x) => x.id !== own.id);
  };
  await remove();
  record({ label: m.undo_snippet_deleted({ name: own.name }), toast: true, undo: () => restoreSnippet(own), redo: remove, valid: (direction) => hasSnippet(own.id) === (direction === 'redo') });
}

export const noteOf = (scope: NoteScope, scopeId: string) => library.notes.find((n) => n.scope === scope && n.scope_id === scopeId) ?? null;

export async function saveNote(scope: NoteScope, scopeId: string, body: string) {
  const saved = await api.noteSet(scope, scopeId, body);
  const rest = library.notes.filter((n) => !(n.scope === scope && n.scope_id === scopeId));
  library.notes = saved ? [saved, ...rest] : rest;
  return saved;
}
