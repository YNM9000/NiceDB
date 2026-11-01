import { m } from '../paraglide/messages';
import type { Command } from './palette.svelte';

export type ValueMode = 'value' | 'record';
export type Surface = 'results' | 'data';

export const valuePane = $state({ results: false, data: false, mode: 'value' as ValueMode });

export function toggleValue(surface: Surface, mode: ValueMode = valuePane.mode): boolean {
  if (valuePane[surface] && valuePane.mode === mode) {
    valuePane[surface] = false;
    return false;
  }
  valuePane.mode = mode;
  valuePane[surface] = true;
  return true;
}

export function valueKey(e: KeyboardEvent, toggle: (mode: ValueMode) => void): boolean {
  if (e.key !== 'Enter' || e.ctrlKey || e.metaKey || e.shiftKey === e.altKey) return false;
  e.preventDefault();
  toggle(e.shiftKey ? 'value' : 'record');
  return true;
}

export function valueCommands(when: () => boolean, toggle: (mode: ValueMode) => void): Command[] {
  const group = m.pal_group_panels;
  return [
    { id: 'panel.value', label: m.vp_cmd_value, group, keys: () => 'Shift+Enter', when, run: () => toggle('value') },
    { id: 'panel.record', label: m.vp_cmd_record, group, keys: () => 'Alt+Enter', when, run: () => toggle('record') },
  ];
}
