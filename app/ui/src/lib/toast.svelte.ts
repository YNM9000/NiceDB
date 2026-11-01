import { play } from './sound';

export interface ToastAction {
  label: string;
  run: () => unknown;
}

export interface Toast {
  id: number;
  text: string;
  action: ToastAction | null;
  tone: 'info' | 'error';
}

export const toasts = $state({ current: null as Toast | null });

const DEFAULT_MS = 6000;
let seq = 0;
let timer: ReturnType<typeof setTimeout> | undefined;
let left = 0;
let startedAt = 0;

function arm(ms: number) {
  clearTimeout(timer);
  left = ms;
  startedAt = Date.now();
  timer = setTimeout(dismiss, ms);
}

export function showToast(text: string, action: ToastAction | null = null, tone: Toast['tone'] = 'info', ms = DEFAULT_MS) {
  play(tone === 'error' ? 'error' : 'notify');
  toasts.current = { id: ++seq, text, action, tone };
  arm(ms);
}

export function dismiss() {
  clearTimeout(timer);
  toasts.current = null;
}

export function pauseToast() {
  if (!toasts.current) return;
  clearTimeout(timer);
  left = Math.max(1500, left - (Date.now() - startedAt));
}

export function resumeToast() {
  if (toasts.current) arm(left);
}
