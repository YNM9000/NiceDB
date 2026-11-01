import type { ParamKind, ParamSlot, ParamUse, ParamValue } from './bindings';

export interface ParamEntry {
  kind: ParamKind;
  value: string;
  chosen: boolean;
}

export const KINDS: readonly ParamKind[] = ['text', 'number', 'date', 'boolean', 'null', 'raw'];

const NUMBER = /^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/;
const DATE = /^\d{4}-\d{2}-\d{2}([ T][\d:.+\-Z]*)?$/;
const NAME = /^[A-Za-z_][\w$]*(\.[A-Za-z_][\w$]*)*$/;
const TRUE = ['true', '1', 't', 'yes', 'on', 'sì', 'si', 'vero'];
const FALSE = ['false', '0', 'f', 'no', 'off', 'falso'];

export function guessKind(value: string): ParamKind {
  const v = value.trim();
  if (NUMBER.test(v)) return 'number';
  if (DATE.test(v)) return 'date';
  if (/^(true|false)$/i.test(v)) return 'boolean';
  return 'text';
}

export function fits(e: Pick<ParamEntry, 'kind' | 'value'>): boolean {
  const v = e.value.trim().toLowerCase();
  switch (e.kind) {
    case 'number':
      return NUMBER.test(v);
    case 'date':
      return DATE.test(v);
    case 'boolean':
      return TRUE.includes(v) || FALSE.includes(v);
    case 'raw':
      return NAME.test(e.value.trim());
    default:
      return true;
  }
}

export function isOn(value: string): boolean {
  return TRUE.includes(value.trim().toLowerCase());
}

export function slotLabel(slot: Pick<ParamSlot, 'name' | 'label'>): string {
  return slot.label === '?' ? `?${slot.name}` : slot.label;
}

export function nameLabel(name: string, slots: readonly ParamSlot[]): string {
  const slot = slots.find((s) => s.name === name);
  return slot ? slotLabel(slot) : /^\d+$/.test(name) ? `?${name}` : `:${name}`;
}

export function historyOf(name: string, history: readonly ParamUse[]): ParamUse[] {
  const seen = new Set<string>();
  return history.filter((h) => {
    const key = `${h.kind}\u0000${h.value}`;
    if (h.name !== name || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function startEntry(name: string, history: readonly ParamUse[]): ParamEntry {
  const last = historyOf(name, history).find((h) => h.here);
  return last ? { kind: last.kind, value: last.value, chosen: true } : { kind: 'text', value: '', chosen: false };
}

export function fillEntries(entries: Record<string, ParamEntry>, slots: readonly ParamSlot[], history: readonly ParamUse[]): string[] {
  const added: string[] = [];
  for (const s of slots) {
    if (entries[s.name]) continue;
    entries[s.name] = startEntry(s.name, history);
    added.push(s.name);
  }
  return added;
}

export function unset(e: ParamEntry | undefined): boolean {
  return !e || (!e.chosen && e.value === '');
}

export function typed(entry: ParamEntry, value: string): ParamEntry {
  return { ...entry, value, kind: entry.chosen ? entry.kind : guessKind(value) };
}

export function valuesFor(slots: readonly ParamSlot[], entries: Record<string, ParamEntry>): ParamValue[] {
  return slots.flatMap((s) => {
    const e = entries[s.name];
    return e ? [{ name: s.name, kind: e.kind, value: e.kind === 'boolean' ? String(isOn(e.value)) : e.value }] : [];
  });
}

export function paramsSummary(values: readonly ParamValue[]): string {
  return values.map((v) => `${/^\d+$/.test(v.name) ? `?${v.name}` : `:${v.name}`} = ${v.kind === 'null' ? 'NULL' : v.value}`).join(' · ');
}
