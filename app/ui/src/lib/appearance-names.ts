import { m } from '../paraglide/messages';
import type { Appearance } from './api';

type Style = Appearance['style'];
type Palette = Appearance['palette'];

export const styleName: Record<Style, () => string> = { registro: m.style_registro, tavola: m.style_tavola, banco: m.style_banco, listino: m.style_listino };

export const styleDesc: Record<Style, () => string> = {
  registro: m.style_registro_desc,
  tavola: m.style_tavola_desc,
  banco: m.style_banco_desc,
  listino: m.style_listino_desc,
};

const PALETTE_NAME: Record<`${Style}_${Palette}`, () => string> = {
  registro_1: m.palette_registro_1,
  registro_2: m.palette_registro_2,
  registro_3: m.palette_registro_3,
  tavola_1: m.palette_tavola_1,
  tavola_2: m.palette_tavola_2,
  tavola_3: m.palette_tavola_3,
  banco_1: m.palette_banco_1,
  banco_2: m.palette_banco_2,
  banco_3: m.palette_banco_3,
  listino_1: m.palette_listino_1,
  listino_2: m.palette_listino_2,
  listino_3: m.palette_listino_3,
};

export const paletteName = (style: Style, palette: Palette) => PALETTE_NAME[`${style}_${palette}`]();

export const themeLabel: Record<Appearance['theme'], () => string> = { system: m.theme_system, light: m.theme_light, dark: m.theme_dark };

export const localeName = (l: Appearance['locale']) => (l === 'it' ? 'Italiano' : l === 'en' ? 'English' : '简体中文');

export function arrowPick<T>(e: KeyboardEvent, list: readonly T[], current: T, pick: (v: T) => void) {
  const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
  if (!step) return;
  e.preventDefault();
  const next = list[(list.indexOf(current) + step + list.length) % list.length];
  if (next === undefined) return;
  pick(next);
  const group = e.currentTarget as HTMLElement | null;
  queueMicrotask(() => group?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus());
}
