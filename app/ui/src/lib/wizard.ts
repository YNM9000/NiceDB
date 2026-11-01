export const STEPS = ['language', 'look', 'comfort', 'safety', 'ready'] as const;
export type Step = (typeof STEPS)[number];

export const LAST = STEPS.length - 1;

export const clampStep = (i: number) => Math.min(LAST, Math.max(0, i));

export type Intent = 'next' | 'skip' | null;

interface KeyTarget {
  tagName: string;
  getAttribute(name: string): string | null;
}

export function keyIntent(e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey' | 'isComposing'>, target: KeyTarget | null): Intent {
  if (e.isComposing || e.ctrlKey || e.metaKey || e.altKey) return null;
  if (e.key === 'Escape') return 'skip';
  if (e.key !== 'Enter' || e.shiftKey) return null;
  const tag = target?.tagName.toLowerCase() ?? '';
  if (tag === 'textarea' || tag === 'select' || tag === 'a') return null;
  if (tag === 'button' && target?.getAttribute('role') !== 'radio') return null;
  return 'next';
}

export const showWizard = (setupDone: boolean, reopened: boolean) => reopened || !setupDone;
