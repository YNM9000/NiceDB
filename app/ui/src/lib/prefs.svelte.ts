import { m } from '../paraglide/messages';
import { api, ApiError, type Preferences } from './api';
import { askConfirm } from './guard.svelte';
import { defaultPreferences } from './prefs';
import { bindings, type Bindings } from './shortcuts';

export const prefs = $state({ value: structuredClone(defaultPreferences) as Preferences });

export type Section = 'editor' | 'results' | 'safety';

export async function loadPrefs() {
  prefs.value = await api.preferencesGet();
}

async function setThroughGate(next: Preferences, chosen: boolean): Promise<Preferences | null> {
  try {
    return await api.preferencesSet(next, chosen ? '' : null);
  } catch (e) {
    if (!(e instanceof ApiError) || e.error.code !== 'needs_confirmation') throw e;
    const answer = await askConfirm({
      title: m.prefs_loosen_title(),
      connection: '',
      production: false,
      lines: [m.prefs_loosen_line()],
      risks: [],
      note: m.prefs_loosen_note(),
      action: m.prefs_loosen_btn(),
    });
    if (answer === false) return null;
    return api.preferencesSet(next, answer ?? '');
  }
}

export async function savePrefs(next: Preferences, chosen = false) {
  const previous = prefs.value;
  prefs.value = next;
  try {
    prefs.value = (await setThroughGate($state.snapshot(next) as Preferences, chosen)) ?? previous;
  } catch (e) {
    prefs.value = previous;
    throw e;
  }
}

export function updateSection<S extends Section>(section: S, patch: Partial<Preferences[S]>, chosen = false) {
  return savePrefs({ ...prefs.value, [section]: { ...prefs.value[section], ...patch } }, chosen);
}

export const setKeys = (keys: Record<string, string>) => savePrefs({ ...prefs.value, keys });

export function resetPrefs(part: Section | 'keys' | 'all') {
  const fresh = structuredClone(defaultPreferences);
  return savePrefs(part === 'all' ? fresh : { ...prefs.value, [part]: fresh[part] });
}

export const currentBindings = (): Bindings => bindings(prefs.value.keys);
