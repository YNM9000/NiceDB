export type ActionId = 'run_statement' | 'run_script' | 'dry_run' | 'cancel' | 'format' | 'rename_alias' | 'save_script' | 'panel_notes' | 'panel_history' | 'new_tab' | 'palette' | 'palette_commands';

export interface ShortcutAction {
  id: ActionId;
  area: 'editor' | 'app';
  combo: string;
}

export const ACTIONS: readonly ShortcutAction[] = [
  { id: 'run_statement', area: 'editor', combo: 'Mod+Enter' },
  { id: 'run_script', area: 'editor', combo: 'Shift+Mod+Enter' },
  { id: 'dry_run', area: 'editor', combo: 'Alt+Mod+Enter' },
  { id: 'cancel', area: 'editor', combo: 'Escape' },
  { id: 'format', area: 'editor', combo: 'Shift+Alt+F' },
  { id: 'rename_alias', area: 'editor', combo: 'Shift+F6' },
  { id: 'save_script', area: 'app', combo: 'Shift+Mod+S' },
  { id: 'panel_notes', area: 'app', combo: 'Alt+Mod+N' },
  { id: 'panel_history', area: 'app', combo: 'Alt+Mod+H' },
  { id: 'new_tab', area: 'app', combo: 'Alt+Mod+T' },
  { id: 'palette', area: 'app', combo: 'Mod+K' },
  { id: 'palette_commands', area: 'app', combo: 'Shift+Mod+P' },
];

export const FIXED: readonly { key: string; combo: string }[] = [
  { key: 'undo', combo: 'Mod+Z' },
  { key: 'redo', combo: 'Shift+Mod+Z' },
  { key: 'redo_alt', combo: 'Mod+Y' },
  { key: 'space', combo: 'Alt+Ctrl+1' },
  { key: 'copy', combo: 'Mod+C' },
  { key: 'paste', combo: 'Mod+V' },
  { key: 'cut', combo: 'Mod+X' },
  { key: 'select_all', combo: 'Mod+A' },
  { key: 'find', combo: 'Mod+F' },
];

const ORDER = ['Shift', 'Alt', 'Mod'] as const;
const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'CapsLock']);

export function comboOf(e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey' | 'code'>): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const parts: string[] = [];
  if (e.shiftKey) parts.push('Shift');
  if (e.altKey) parts.push('Alt');
  if (e.ctrlKey || e.metaKey) parts.push('Mod');
  let key = e.key;
  if (e.altKey && /^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (e.altKey && /^Digit\d$/.test(e.code)) key = e.code.slice(5);
  if (key === ' ') key = 'Space';
  if (key.length === 1) key = key.toUpperCase();
  return [...parts, key].join('+');
}

export function normalize(combo: string): string {
  const parts = combo.split('+').filter(Boolean);
  const key = parts.filter((p) => !(ORDER as readonly string[]).includes(p) && p !== 'Ctrl' && p !== 'Cmd').at(-1) ?? '';
  const mods = new Set(parts.map((p) => (p === 'Ctrl' || p === 'Cmd' ? 'Mod' : p)));
  return [...ORDER.filter((o) => mods.has(o)), key.length === 1 ? key.toUpperCase() : key].join('+');
}

export function matches(e: KeyboardEvent, combo: string): boolean {
  return !!combo && comboOf(e) === normalize(combo);
}

export function forEditor(combo: string): string {
  return normalize(combo)
    .split('+')
    .map((p) => (p.length === 1 ? p.toLowerCase() : p))
    .join('-');
}

export type Bindings = Record<ActionId, string>;

export function bindings(overrides: Record<string, string>): Bindings {
  const out = {} as Bindings;
  for (const a of ACTIONS) out[a.id] = a.id in overrides ? normalize(overrides[a.id] ?? '') : a.combo;
  return out;
}

export type Conflict = { kind: 'action'; id: ActionId } | { kind: 'fixed'; key: string };

export function findConflict(current: Bindings, id: ActionId, combo: string): Conflict | null {
  const c = normalize(combo);
  if (!c) return null;
  const fixed = FIXED.find((f) => normalize(f.combo) === c);
  if (fixed) return { kind: 'fixed', key: fixed.key };
  const other = ACTIONS.find((a) => a.id !== id && current[a.id] === c);
  return other ? { kind: 'action', id: other.id } : null;
}

export function usable(combo: string): boolean {
  const parts = normalize(combo).split('+');
  const key = parts.at(-1) ?? '';
  return parts.length > 1 || key === 'Escape' || /^F\d{1,2}$/.test(key);
}

export function overridesAfter(overrides: Record<string, string>, id: ActionId, combo: string | null): Record<string, string> {
  const next = { ...overrides };
  const base = ACTIONS.find((a) => a.id === id)?.combo;
  if (combo === null || (combo !== '' && normalize(combo) === base)) delete next[id];
  else next[id] = combo === '' ? '' : normalize(combo);
  return next;
}

export function shown(combo: string): string {
  const parts = normalize(combo).split('+');
  const key = parts.pop() ?? '';
  const has = (m: string) => parts.includes(m);
  return [...(has('Mod') ? ['Mod'] : []), ...(has('Alt') ? ['Alt'] : []), ...(has('Shift') ? ['Shift'] : []), key].join('+');
}
