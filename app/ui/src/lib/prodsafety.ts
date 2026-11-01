import type { ConfirmMode, UnlockMode } from './bindings';

export const UNLOCK_MODES: readonly UnlockMode[] = ['timed', 'session', 'open'];
export const CONFIRM_MODES: readonly ConfirmMode[] = ['typed', 'modal', 'none'];

export type GateError = 'read_only' | 'needs_confirmation' | 'name_mismatch';

export function gate(mode: ConfirmMode, open: boolean, name: string, confirm: string | null | undefined): GateError | null {
  if (!open) return 'read_only';
  if (mode === 'none') return null;
  if (confirm === null || confirm === undefined) return 'needs_confirmation';
  if (mode === 'typed' && confirm.trim() !== name.trim()) return 'name_mismatch';
  return null;
}

export function confirmAnswer(mode: ConfirmMode, production: boolean, typed: string): string | null {
  if (!production || mode === 'none') return null;
  return mode === 'typed' ? typed.trim() : '';
}

export const asksForConfirmation = (mode: ConfirmMode, production: boolean) => !production || mode !== 'none';

const UNLOCK_RANK: Record<UnlockMode, number> = { timed: 0, session: 1, open: 2 };
const CONFIRM_RANK: Record<ConfirmMode, number> = { typed: 0, modal: 1, none: 2 };

export function loosens(before: { unlock_mode: UnlockMode; confirm_mode: ConfirmMode }, after: { unlock_mode: UnlockMode; confirm_mode: ConfirmMode }): boolean {
  return UNLOCK_RANK[after.unlock_mode] > UNLOCK_RANK[before.unlock_mode] || CONFIRM_RANK[after.confirm_mode] > CONFIRM_RANK[before.confirm_mode];
}
