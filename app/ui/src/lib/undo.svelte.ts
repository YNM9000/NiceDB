import { m } from '../paraglide/messages';
import { showToast } from './toast.svelte';

export interface UndoEntry {
  label: string;
  undo: () => unknown;
  redo: () => unknown;
  valid?: (direction: 'undo' | 'redo') => boolean;
  expire?: () => unknown;
  ttl?: number;
  key?: string;
  toast?: boolean;
}

const LIMIT = 100;
const COALESCE_MS = 1500;

class Stacks {
  undo = $state.raw<UndoEntry[]>([]);
  redo = $state.raw<UndoEntry[]>([]);
}

export const stacks = new Stacks();

const timers = new Map<UndoEntry, ReturnType<typeof setTimeout>>();
const stamps = new WeakMap<UndoEntry, number>();
const first: (() => boolean)[] = [];
const again: (() => boolean)[] = [];
let busy = false;

function settle(e: UndoEntry) {
  try {
    void Promise.resolve(e.expire?.()).catch(() => undefined);
  } catch {
    return;
  }
}

function arm(e: UndoEntry) {
  if (!e.ttl) return;
  clearTimeout(timers.get(e));
  timers.set(
    e,
    setTimeout(() => {
      timers.delete(e);
      if (!stacks.undo.includes(e)) return;
      stacks.undo = stacks.undo.filter((x) => x !== e);
      settle(e);
    }, e.ttl),
  );
}

function disarm(e: UndoEntry) {
  clearTimeout(timers.get(e));
  timers.delete(e);
}

export function undoFirst(handler: () => boolean) {
  first.push(handler);
}

export function redoFirst(handler: () => boolean) {
  again.push(handler);
}

export function record(entry: UndoEntry) {
  const now = Date.now();
  const top = stacks.undo.at(-1);
  stacks.redo = [];
  if (entry.key && top?.key === entry.key && now - (stamps.get(top) ?? 0) < COALESCE_MS) {
    const merged: UndoEntry = { ...entry, undo: top.undo };
    stamps.set(merged, now);
    stacks.undo = [...stacks.undo.slice(0, -1), merged];
    return merged;
  }
  stamps.set(entry, now);
  let next = [...stacks.undo, entry];
  while (next.length > LIMIT) {
    const old = next[0];
    next = next.slice(1);
    if (old) {
      disarm(old);
      settle(old);
    }
  }
  stacks.undo = next;
  arm(entry);
  if (entry.toast) showToast(entry.label, { label: m.undo_action(), run: () => undoEntry(entry) });
  return entry;
}

function announce(text: string, action: { label: string; run: () => unknown }) {
  showToast(text, action);
}

async function apply(entry: UndoEntry, forward: boolean) {
  try {
    await (forward ? entry.redo() : entry.undo());
    return true;
  } catch (e) {
    showToast(m.undo_failed({ what: entry.label, detail: e instanceof Error ? e.message : String(e) }), null, 'error');
    return false;
  }
}

export async function undoEntry(entry: UndoEntry): Promise<boolean> {
  if (busy || !stacks.undo.includes(entry)) return false;
  busy = true;
  try {
    stacks.undo = stacks.undo.filter((x) => x !== entry);
    disarm(entry);
    if (entry.valid && !entry.valid('undo')) {
      settle(entry);
      return false;
    }
    if (!(await apply(entry, false))) return false;
    stacks.redo = [...stacks.redo, entry];
    announce(m.undo_done({ what: entry.label }), { label: m.redo_action(), run: redo });
    return true;
  } finally {
    busy = false;
  }
}

export async function undo(): Promise<boolean> {
  if (busy) return false;
  if (first.some((h) => h())) return true;
  for (;;) {
    const top = stacks.undo.at(-1);
    if (!top) return false;
    if (top.valid && !top.valid('undo')) {
      stacks.undo = stacks.undo.slice(0, -1);
      disarm(top);
      settle(top);
      continue;
    }
    return undoEntry(top);
  }
}

export async function redo(): Promise<boolean> {
  if (busy) return false;
  if (again.some((h) => h())) return true;
  busy = true;
  try {
    for (;;) {
      const top = stacks.redo.at(-1);
      if (!top) return false;
      stacks.redo = stacks.redo.slice(0, -1);
      if (top.valid && !top.valid('redo')) continue;
      if (!(await apply(top, true))) return false;
      stamps.set(top, Date.now());
      stacks.undo = [...stacks.undo, top];
      arm(top);
      announce(m.redo_done({ what: top.label }), { label: m.undo_action(), run: () => undoEntry(top) });
      return true;
    }
  } finally {
    busy = false;
  }
}

export const canUndo = () => stacks.undo.length > 0;
export const canRedo = () => stacks.redo.length > 0;

export function forget() {
  for (const e of stacks.undo) {
    disarm(e);
    settle(e);
  }
  stacks.undo = [];
  stacks.redo = [];
}

const TEXT_INPUTS = /^(text|search|url|email|tel|password|number|date|time|datetime-local)$/;

export function typing(t: EventTarget | null): boolean {
  const el = t as { tagName?: string; type?: string; isContentEditable?: boolean; closest?: (s: string) => unknown } | null;
  if (!el?.tagName) return false;
  if (el.isContentEditable || el.closest?.('.cm-editor')) return true;
  if (el.tagName === 'TEXTAREA') return true;
  return el.tagName === 'INPUT' && TEXT_INPUTS.test(el.type ?? 'text');
}

export function undoKeys(target: Window = window) {
  const onkey = (e: KeyboardEvent) => {
    if (e.defaultPrevented || e.altKey || !(e.ctrlKey || e.metaKey) || typing(e.target)) return;
    if (document.querySelector('dialog[open]')) return;
    const k = e.key.toLowerCase();
    const wantsRedo = (k === 'z' && e.shiftKey) || (k === 'y' && !e.shiftKey);
    if (k !== 'z' && k !== 'y') return;
    if (k === 'y' && e.shiftKey) return;
    e.preventDefault();
    if (e.repeat) return;
    void (wantsRedo ? redo() : undo());
  };
  target.addEventListener('keydown', onkey);
  return () => target.removeEventListener('keydown', onkey);
}
