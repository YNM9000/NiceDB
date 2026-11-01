import type { Component } from 'svelte';
import { m } from '../paraglide/messages';
import { play } from './sound';

export interface MenuAction {
  label: string;
  icon?: Component<{ size?: number; strokeWidth?: number }>;
  shortcut?: string;
  hint?: string;
  disabled?: boolean;
  danger?: boolean;
  run?: () => unknown;
  items?: MenuEntry[];
}

export type MenuEntry = MenuAction | 'separator' | null | undefined | false;

export interface OpenMenu {
  x: number;
  y: number;
  label: string;
  items: MenuAction[][];
  restore: HTMLElement | null;
  seq: number;
}

export const contextMenu = $state<{ open: OpenMenu | null }>({ open: null });

let seq = 0;

const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);

export function keys(combo: string): string {
  return combo
    .split('+')
    .map((k) => (k === 'Mod' ? (isMac ? '⌘' : 'Ctrl') : k === 'Shift' ? '⇧' : k === 'Alt' ? (isMac ? '⌥' : 'Alt') : k === 'Enter' ? '↵' : k))
    .join(isMac ? '' : '+');
}

export function groups(entries: readonly MenuEntry[]): MenuAction[][] {
  const out: MenuAction[][] = [[]];
  for (const e of entries) {
    if (!e) continue;
    if (e === 'separator') out.push([]);
    else out.at(-1)?.push(e);
  }
  return out.filter((g) => g.length > 0);
}

function origin(e: MouseEvent | { x: number; y: number }): { x: number; y: number } {
  if (!(e instanceof MouseEvent)) return e;
  if (e.clientX !== 0 || e.clientY !== 0) return { x: e.clientX, y: e.clientY };
  const el = e.target instanceof Element ? e.target : document.activeElement;
  const r = el?.getBoundingClientRect();
  return r ? { x: r.left + Math.min(16, r.width / 2), y: r.top + Math.min(r.height, 28) } : { x: 0, y: 0 };
}

export function openMenu(e: MouseEvent | { x: number; y: number }, entries: readonly MenuEntry[], label = m.menu_label()) {
  if (e instanceof MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
  }
  const items = groups(entries);
  if (items.length === 0) return;
  const at = origin(e);
  const active = document.activeElement;
  contextMenu.open = { ...at, label, items, restore: active instanceof HTMLElement ? active : null, seq: ++seq };
}

export function closeMenu(restore = true) {
  const open = contextMenu.open;
  if (!open) return;
  contextMenu.open = null;
  if (restore && open.restore?.isConnected) open.restore.focus({ preventScroll: true });
}

export async function copyText(text: string) {
  await navigator.clipboard.writeText(text);
  play('copy');
}

type Editable = HTMLInputElement | HTMLTextAreaElement;

const TEXT_INPUTS = /^(text|search|url|email|tel|password|number)$/;

function editable(t: EventTarget | null): Editable | null {
  if (t instanceof HTMLTextAreaElement) return t;
  if (t instanceof HTMLInputElement && TEXT_INPUTS.test(t.type)) return t;
  return null;
}

function textMenu(el: Editable): MenuEntry[] {
  const from = el.selectionStart ?? 0;
  const to = el.selectionEnd ?? 0;
  const selected = to > from && el.type !== 'password';
  const restore = () => {
    el.focus({ preventScroll: true });
    el.setSelectionRange(from, to);
  };
  const writable = !el.readOnly && !el.disabled;
  return [
    {
      label: m.menu_cut(),
      shortcut: keys('Mod+X'),
      disabled: !selected || !writable,
      run: () => {
        restore();
        void copyText(el.value.slice(from, to));
        document.execCommand('delete');
      },
    },
    {
      label: m.menu_copy(),
      shortcut: keys('Mod+C'),
      disabled: !selected,
      run: () => void copyText(el.value.slice(from, to)),
    },
    {
      label: m.menu_paste(),
      shortcut: keys('Mod+V'),
      disabled: !writable,
      run: async () => {
        const text = await navigator.clipboard.readText().catch(() => '');
        restore();
        if (text) document.execCommand('insertText', false, text);
      },
    },
    'separator',
    {
      label: m.menu_select_all(),
      shortcut: keys('Mod+A'),
      disabled: el.value.length === 0,
      run: () => {
        el.focus({ preventScroll: true });
        el.select();
      },
    },
  ];
}

export function guardNativeMenu(target: Window = window) {
  const onmenu = (e: MouseEvent) => {
    if (e.defaultPrevented) return;
    e.preventDefault();
    const el = editable(e.target);
    if (el) openMenu(e, textMenu(el));
  };
  target.addEventListener('contextmenu', onmenu);
  return () => target.removeEventListener('contextmenu', onmenu);
}
