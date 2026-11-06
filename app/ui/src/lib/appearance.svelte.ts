import { overwriteGetLocale } from '../paraglide/runtime';
import { api, defaultAppearance, type Appearance } from './api';
import { configureSound } from './sound';

const voice = (a: Appearance) =>
  configureSound({ on: a.sounds, volume: a.sound_volume, reduced: a.sound_detail === 'reduced', timbre: a.sound_timbre, gestures: a.sound_gestures, jingle: a.jingle });

export const STYLES = ['registro', 'tavola', 'banco', 'listino'] as const;
export const PALETTES = ['1', '2', '3'] as const;
export const THEMES = ['system', 'light', 'dark'] as const;
export const LOCALES = ['it', 'en', 'zh-CN'] as const;
export const SCALES = [90, 100, 110, 125, 150] as const;

const dark = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null;

export const appearance = $state({ value: { ...defaultAppearance }, systemDark: dark?.matches ?? true });

overwriteGetLocale(() => appearance.value.locale);

export function resolvedTheme(a: Appearance, systemDark: boolean): 'light' | 'dark' {
  return a.theme === 'system' ? (systemDark ? 'dark' : 'light') : a.theme;
}

export function applyToDocument(a: Appearance, systemDark: boolean, root: HTMLElement = document.documentElement) {
  root.dataset.style = a.style;
  root.dataset.palette = a.palette;
  root.dataset.theme = resolvedTheme(a, systemDark);
  root.lang = a.locale;
  if (root !== document.documentElement) return;
  root.style.background = '';
  requestAnimationFrame(() => {
    try {
      localStorage.setItem('nd-look', JSON.stringify({ s: a.style, p: a.palette, t: root.dataset.theme, bg: getComputedStyle(document.body).backgroundColor }));
    } catch {}
  });
}

export async function loadAppearance() {
  appearance.value = await api.appearanceGet();
  applyToDocument(appearance.value, appearance.systemDark);
  voice(appearance.value);
  dark?.addEventListener('change', (e) => {
    appearance.systemDark = e.matches;
    applyToDocument(appearance.value, e.matches);
  });
}

export async function updateAppearance(patch: Partial<Appearance>, { replaySplash = true } = {}) {
  const previous = appearance.value;
  appearance.value = { ...previous, ...patch };
  applyToDocument(appearance.value, appearance.systemDark);
  voice(appearance.value);
  if (replaySplash && appearance.value.style !== previous.style) void import('./play-splash').then((m) => m.playSplash());
  try {
    await api.appearanceSet(appearance.value);
  } catch (e) {
    appearance.value = previous;
    applyToDocument(previous, appearance.systemDark);
    voice(previous);
    throw e;
  }
}
