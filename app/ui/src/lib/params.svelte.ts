import { api, paramFault } from './api';
import type { Dialect, ParamSlot, ParamUse, ParamValue, RunScope } from './bindings';
import { fillEntries, fits, slotLabel, unset, valuesFor, type ParamEntry } from './params';

export class TabParams {
  slots = $state.raw<ParamSlot[]>([]);
  history = $state.raw<ParamUse[]>([]);
  entries = $state<Record<string, ParamEntry>>({});
  asked = $state.raw<string[]>([]);
  fault = $state.raw<{ name: string; message: string } | null>(null);
  pending: { scope: RunScope; dry: boolean } | null = null;
  seq = 0;
}

const tabs = new Map<string, TabParams>();

export function paramsOf(tabId: string): TabParams {
  let p = tabs.get(tabId);
  if (!p) {
    p = new TabParams();
    tabs.set(tabId, p);
  }
  return p;
}

export function seedParams(tabId: string, values: readonly ParamValue[]) {
  const p = paramsOf(tabId);
  for (const v of values) p.entries[v.name] = { kind: v.kind, value: v.value, chosen: true };
}

export function savedParams(tabId: string): ParamValue[] {
  const p = tabs.get(tabId);
  if (!p) return [];
  return Object.entries(p.entries)
    .filter(([, e]) => e.chosen || e.value !== '')
    .map(([name, e]) => ({ name, kind: e.kind, value: e.value }));
}

export function paramsIn(tabId: string, sql: string): ParamValue[] {
  const p = tabs.get(tabId);
  if (!p) return [];
  return Object.entries(p.entries)
    .filter(([name]) => (/^\d+$/.test(name) ? sql.includes('?') || sql.includes(`$${name}`) : new RegExp(`(:|\\$\\{|@|\\$)${name}\\b`).test(sql)))
    .map(([name, e]) => ({ name, kind: e.kind, value: e.value }));
}

export function clearParams(tabId: string) {
  const p = paramsOf(tabId);
  p.entries = Object.fromEntries(p.slots.map((s) => [s.name, { kind: 'text' as const, value: '', chosen: false }]));
  p.asked = [];
  p.fault = null;
  p.seq++;
}

async function find(text: string, dialect: Dialect, scope: RunScope) {
  if (!/[:?$@]/.test(text)) return { slots: [], history: [] };
  return api.paramsFound(text, dialect, scope);
}

export async function refreshParams(tabId: string, text: string, dialect: Dialect, scope: RunScope) {
  const p = paramsOf(tabId);
  const seq = ++p.seq;
  p.pending = null;
  const found = await find(text, dialect, scope).catch(() => null);
  if (!found || seq !== p.seq) return;
  fillEntries(p.entries, found.slots, found.history);
  const same = found.slots.length === p.slots.length && found.slots.every((s, i) => s.name === p.slots[i]?.name && s.label === p.slots[i]?.label && s.spots.length === p.slots[i]?.spots.length && s.spots.every((x, j) => x.from === p.slots[i]?.spots[j]?.from));
  if (!same) p.slots = found.slots;
  p.history = found.history;
  if (p.asked.length && !found.slots.some((s) => p.asked.includes(s.name))) p.asked = [];
}

export async function paramsForRun(tabId: string, text: string, dialect: Dialect, scope: RunScope, dry = false): Promise<ParamValue[] | null | false> {
  const p = paramsOf(tabId);
  const found = await find(text, dialect, scope);
  if (!found.slots.length) return null;
  fillEntries(p.entries, found.slots, found.history);
  const fresh = found.slots.map((s) => s.name).filter((n) => unset(p.entries[n]));
  p.fault = null;
  if (fresh.length) {
    p.seq++;
    p.slots = found.slots;
    p.history = found.history;
    p.asked = fresh;
    p.pending = { scope, dry };
    return false;
  }
  p.asked = [];
  const wrong = found.slots.find((s) => !fits(p.entries[s.name] ?? { kind: 'text', value: '' }));
  const entry = wrong && p.entries[wrong.name];
  if (wrong && entry) {
    p.fault = { name: wrong.name, message: paramFault({ kind: 'invalid', name: wrong.name, as_kind: entry.kind }, slotLabel(wrong)) };
    p.pending = { scope, dry };
    p.slots = found.slots;
    return false;
  }
  p.pending = null;
  return valuesFor(found.slots, p.entries);
}
